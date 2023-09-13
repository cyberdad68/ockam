use minicbor::{Decode, Encode};
use std::sync::Arc;
use std::time::Duration;

use ockam::identity::{IdentityIdentifier, SecureChannelOptions, TrustIdentifierPolicy};
use ockam_core::api::{Reply, Request, Response};
use ockam_core::{self, route, Result, Route};
use ockam_identity::SecureChannels;
use ockam_node::api::request_with_options;
use ockam_node::{Context, MessageSendReceiveOptions, DEFAULT_TIMEOUT};

#[derive(Clone)]
pub struct SecureClient {
    secure_channels: Arc<SecureChannels>,
    server_route: Route,
    server_identifier: IdentityIdentifier,
    client_identifier: IdentityIdentifier,
}

impl SecureClient {
    pub fn new(
        secure_channels: Arc<SecureChannels>,
        server_route: Route,
        server_identifier: IdentityIdentifier,
        client_identifier: IdentityIdentifier,
        timeout: Duration,
    ) -> SecureClient {
        Self {
            secure_channels,
            server_route,
            server_identifier,
            client_identifier,
            timeout
        }
    }
}

impl SecureClient {
    pub async fn ask<T, R>(
        &self,
        ctx: &Context,
        api_service: &str,
        req: Request<T>,
    ) -> Result<Reply<R>>
        where
            T: Encode<()>,
            R: for<'a> Decode<'a, ()>,
    {
        let bytes = self
            .request_with_timeout(
                ctx,
                api_service,
                req,
                self.timeout,
            )
            .await?;
        Response::parse_response_reply::<R>(&bytes)
    }

    pub async fn tell<T>(
        &self,
        ctx: &Context,
        api_service: &str,
        req: Request<T>,
    ) -> Result<Reply<()>>
        where
            T: Encode<()>,
    {
        let request_header = req.header().clone();
        let bytes = self
            .request_with_timeout(
                ctx,
                api_service,
                req,
                self.timeout,
            )
            .await?;
        let (response, decoder) = Response::parse_response_header(bytes.as_slice())?;
        if !response.is_ok() {
            Ok(Reply::Failed(Error::from_failed_request(&request_header, &response.parse_err_msg(decoder)), response.status()))
        } else {
            Ok(Successful(()))
        }
    }

    pub async fn request<T>(
        &self,
        ctx: &Context,
        api_service: &str,
        req: Request<T>,
    ) -> Result<Vec<u8>>
    where
        T: Encode<()>,
    {
        self.request_with_timeout(
            ctx,
            api_service,
            req,
            self.timeout,
        )
        .await
    }

    pub async fn request_with_timeout<T>(
        &self,
        ctx: &Context,
        api_service: &str,
        req: Request<T>,
        timeout: Duration,
    ) -> Result<Vec<u8>>
    where
        T: Encode<()>,
    {
        let options = SecureChannelOptions::new()
            .with_trust_policy(TrustIdentifierPolicy::new(self.server_identifier.clone()));
        let sc = self
            .secure_channels
            .create_secure_channel(
                ctx,
                &self.client_identifier,
                self.server_route.clone(),
                options,
            )
            .await?;

        let route = route![sc.clone(), api_service];
        let options = MessageSendReceiveOptions::new().with_timeout(timeout);
        let res = request_with_options(ctx, route, req, options).await;
        self.secure_channels
            .stop_secure_channel(ctx, sc.encryptor_address())
            .await?;
        res
    }
}
