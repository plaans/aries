#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Fluent {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub value_type: ::prost::alloc::string::String,
    #[prost(string, repeated, tag = "3")]
    pub signature: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Object {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub r#type: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Expression {
    #[prost(int64, tag = "1")]
    pub r#type: i64,
    #[prost(message, repeated, tag = "2")]
    pub args: ::prost::alloc::vec::Vec<Expression>,
    #[prost(message, optional, tag = "3")]
    pub payload: ::core::option::Option<Payload>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Assignment {
    #[prost(message, optional, tag = "1")]
    pub x: ::core::option::Option<Expression>,
    #[prost(message, optional, tag = "2")]
    pub v: ::core::option::Option<Expression>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Payload {
    #[prost(string, tag = "1")]
    pub r#type: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub value: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Action {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, repeated, tag = "2")]
    pub parameters: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag = "3")]
    pub parameter_types: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(message, repeated, tag = "4")]
    pub preconditions: ::prost::alloc::vec::Vec<Expression>,
    #[prost(message, repeated, tag = "5")]
    pub effects: ::prost::alloc::vec::Vec<Assignment>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Problem {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, repeated, tag = "2")]
    pub fluents: ::prost::alloc::vec::Vec<Fluent>,
    #[prost(message, repeated, tag = "3")]
    pub objects: ::prost::alloc::vec::Vec<Object>,
    #[prost(message, repeated, tag = "4")]
    pub actions: ::prost::alloc::vec::Vec<Action>,
    #[prost(message, repeated, tag = "5")]
    pub initial_state: ::prost::alloc::vec::Vec<Assignment>,
    #[prost(message, repeated, tag = "6")]
    pub goals: ::prost::alloc::vec::Vec<Expression>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ActionInstance {
    #[prost(message, optional, tag = "1")]
    pub action: ::core::option::Option<Action>,
    #[prost(message, repeated, tag = "2")]
    pub parameters: ::prost::alloc::vec::Vec<Expression>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SequentialPlan {
    #[prost(message, repeated, tag = "1")]
    pub actions: ::prost::alloc::vec::Vec<ActionInstance>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Answer {
    #[prost(int32, tag = "1")]
    pub status: i32,
    #[prost(message, optional, tag = "2")]
    pub plan: ::core::option::Option<SequentialPlan>,
}
#[doc = r" Generated client implementations."]
pub mod upf_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    #[derive(Debug, Clone)]
    pub struct UpfClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl UpfClient<tonic::transport::Channel> {
        #[doc = r" Attempt to create a new client by connecting to a given endpoint."]
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> UpfClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::ResponseBody: Body + Send + 'static,
        T::Error: Into<StdError>,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_interceptor<F>(inner: T, interceptor: F) -> UpfClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<<T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody>,
            >,
            <T as tonic::codegen::Service<http::Request<tonic::body::BoxBody>>>::Error: Into<StdError> + Send + Sync,
        {
            UpfClient::new(InterceptedService::new(inner, interceptor))
        }
        #[doc = r" Compress requests with `gzip`."]
        #[doc = r""]
        #[doc = r" This requires the server to support it otherwise it might respond with an"]
        #[doc = r" error."]
        pub fn send_gzip(mut self) -> Self {
            self.inner = self.inner.send_gzip();
            self
        }
        #[doc = r" Enable decompressing responses with `gzip`."]
        pub fn accept_gzip(mut self) -> Self {
            self.inner = self.inner.accept_gzip();
            self
        }
        pub async fn plan(
            &mut self,
            request: impl tonic::IntoRequest<super::Problem>,
        ) -> Result<tonic::Response<super::Answer>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(tonic::Code::Unknown, format!("Service was not ready: {}", e.into()))
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/upf.Upf/plan");
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
}
#[doc = r" Generated server implementations."]
pub mod upf_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    #[doc = "Generated trait containing gRPC methods that should be implemented for use with UpfServer."]
    #[async_trait]
    pub trait Upf: Send + Sync + 'static {
        async fn plan(
            &self,
            request: tonic::Request<super::Problem>,
        ) -> Result<tonic::Response<super::Answer>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct UpfServer<T: Upf> {
        inner: _Inner<T>,
        accept_compression_encodings: (),
        send_compression_encodings: (),
    }
    struct _Inner<T>(Arc<T>);
    impl<T: Upf> UpfServer<T> {
        pub fn new(inner: T) -> Self {
            let inner = Arc::new(inner);
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
            }
        }
        pub fn with_interceptor<F>(inner: T, interceptor: F) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for UpfServer<T>
    where
        T: Upf,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = Never;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/upf.Upf/plan" => {
                    #[allow(non_camel_case_types)]
                    struct planSvc<T: Upf>(pub Arc<T>);
                    impl<T: Upf> tonic::server::UnaryService<super::Problem> for planSvc<T> {
                        type Response = super::Answer;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(&mut self, request: tonic::Request<super::Problem>) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).plan(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = planSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(accept_compression_encodings, send_compression_encodings);
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => Box::pin(async move {
                    Ok(http::Response::builder()
                        .status(200)
                        .header("grpc-status", "12")
                        .header("content-type", "application/grpc")
                        .body(empty_body())
                        .unwrap())
                }),
            }
        }
    }
    impl<T: Upf> Clone for UpfServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
            }
        }
    }
    impl<T: Upf> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: Upf> tonic::transport::NamedService for UpfServer<T> {
        const NAME: &'static str = "upf.Upf";
    }
}
