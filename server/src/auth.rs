use std::{pin::Pin, rc::Rc};

use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use futures::{
    future::{ready, Ready},
    Future,
};

struct SessionMiddleware;

impl<S> Transform<S, ServiceRequest> for SessionMiddleware {
    type Response = ServiceResponse;
    type Error = anyhow::Error;
    type Transform = InnerSessionMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(InnerSessionMiddleware {
            service: Rc::new(service),
        }))
    }
}

struct InnerSessionMiddleware<S> {
    service: Rc<S>,
}

impl<S> Service<ServiceRequest> for InnerSessionMiddleware<S> {
    type Response = ServiceResponse;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &self,
        ctx: &mut core::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        todo!()
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        todo!()
    }
}
