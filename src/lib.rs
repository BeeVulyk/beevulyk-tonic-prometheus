use std::{
    num::NonZeroUsize,
    pin::Pin,
    task::{Context, Poll},
    time::Instant,
};

use pin_project::pin_project;
use tonic::{
    Code,
    codegen::http::{request, response},
};
use tower::{Layer, Service};

use crate::metrics::{COUNTER_MP, COUNTER_SM, COUNTER_SMC, GAUGE_MP, HISTOGRAM_MP, HISTOGRAM_SMC};

pub mod metrics;

#[derive(Clone, Default)]
pub struct MetricsLayer {}

impl MetricsLayer {
    pub fn new() -> Self {
        Default::default()
    }
}

impl<S> Layer<S> for MetricsLayer {
    type Service = MetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MetricsService { service: inner }
    }
}

#[derive(Clone)]
pub struct MetricsService<S> {
    service: S,
}

impl<S, B, C> Service<request::Request<B>> for MetricsService<S>
where
    S: Service<request::Request<B>, Response = response::Response<C>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = MetricsFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: request::Request<B>) -> Self::Future {
        let method = req.method().to_string();
        let path = req.uri().path().to_owned();
        let service_method_separator: Option<NonZeroUsize> = match path.chars().next() {
            Some(first_char) if first_char == '/' => path[1..]
                .find('/')
                .map(|p| NonZeroUsize::new(p + 1).unwrap()),
            _ => None,
        };
        let f = self.service.call(req);

        MetricsFuture::new(method, path, service_method_separator, f)
    }
}

#[pin_project]
pub struct MetricsFuture<F> {
    method: String,
    path: String,
    service_method_separator: Option<NonZeroUsize>,
    started_at: Option<Instant>,
    #[pin]
    inner: F,
}

impl<F> MetricsFuture<F> {
    pub fn new(
        method: String,
        path: String,
        service_method_separator: Option<NonZeroUsize>,
        inner: F,
    ) -> Self {
        Self {
            started_at: None,
            inner,
            method,
            path,
            service_method_separator,
        }
    }
}

impl<F, B, E> Future for MetricsFuture<F>
where
    F: Future<Output = Result<response::Response<B>, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let path = this.path.clone();
        let method = this.method.clone();

        let (rpc_service, rpc_method) = match this.service_method_separator {
            Some(sep) => (
                &this.path[1..(*sep).into()],
                &this.path[usize::from(*sep) + 1..],
            ),
            // If unparseable, say service is empty and method is the entire path
            None => ("", this.path.as_str()),
        };

        let started_at = this.started_at.get_or_insert_with(|| {
            #[cfg(not(feature = "grpc_metrics_disabled"))]
            {
                GAUGE_MP
                    .with_label_values(&[&this.method, &this.path])
                    .inc();
                COUNTER_SM
                    .with_label_values(&[rpc_service, rpc_method])
                    .inc();
            }

            Instant::now()
        });

        if let Poll::Ready(v) = this.inner.poll(cx) {
            #[cfg(not(feature = "grpc_metrics_disabled"))]
            {
                let code = v.as_ref().map_or(Code::Unknown, |resp| {
                    resp.headers()
                        .get("grpc-status")
                        .map(|s| Code::from_bytes(s.as_bytes()))
                        .unwrap_or(Code::Ok)
                });

                let code_str = format!("{:?}", code);
                let elapsed = Instant::now().duration_since(*started_at).as_secs_f64();

                COUNTER_MP.with_label_values(&[&method, &path]).inc();
                COUNTER_SMC
                    .with_label_values(&[rpc_service, rpc_method, &code_str])
                    .inc();
                HISTOGRAM_MP
                    .with_label_values(&[&method, &path])
                    .observe(elapsed);
                HISTOGRAM_SMC
                    .with_label_values(&[rpc_service, rpc_method, &code_str])
                    .observe(elapsed);
                GAUGE_MP.with_label_values(&[&method, &path]).dec();
            }
            Poll::Ready(v)
        } else {
            Poll::Pending
        }
    }
}
