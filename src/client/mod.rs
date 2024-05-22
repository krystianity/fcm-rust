pub(crate) mod internal_client;
pub(crate) mod response;

#[cfg(feature = "gauth")]
pub mod oauth_gauth;

#[cfg(feature = "yup-oauth2")]
pub mod oauth_yup_oauth2;

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::client::response::FcmResponse;
use crate::message::Message;

use self::internal_client::FcmClientInternal;

#[cfg(feature = "gauth")]
pub type DefaultOauthClient = oauth_gauth::Gauth;

#[cfg(all(feature = "yup-oauth2", not(feature = "gauth")))]
pub type DefaultOauthClient = oauth_yup_oauth2::YupOauth2;

const FIREBASE_OAUTH_SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";

#[derive(thiserror::Error, Debug)]
pub enum FcmClientError<T: OauthError = <DefaultOauthClient as OauthClient>::Error> {
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("OAuth error: {0}")]
    Oauth(T),
    #[error("Dotenvy error: {0}")]
    Dotenvy(#[from] dotenvy::Error),
    #[error("Retry-After HTTP header value is not valid string")]
    RetryAfterHttpHeaderIsNotString,
    #[error("Retry-After HTTP header value is not valid, error: {error}, value: {value}")]
    RetryAfterHttpHeaderInvalid {
        error: chrono::ParseError,
        value: String,
    },
}

impl <T: OauthErrorAccessTokenStatus> FcmClientError<T> {
    /// If this is `true` then most likely current service account
    /// key is invalid.
    pub fn is_access_token_missing_even_if_server_requests_completed(&self) -> bool {
        match self {
            FcmClientError::Oauth(error) =>
                error.is_access_token_missing_even_if_server_requests_completed(),
            _ => false,
        }
    }
}

pub trait OauthClient {
    type Error: OauthError;
}

pub(crate) trait OauthClientInternal: OauthClient + Sized {
    fn create_with_key_file(
        service_account_key_path: PathBuf,
        token_cache_json_path: Option<PathBuf>,
    ) -> impl std::future::Future<Output = Result<Self, Self::Error>> + Send;

    fn get_access_token(
        &self
    ) -> impl std::future::Future<Output = Result<String, Self::Error>> + Send;

    fn get_project_id(&self) -> &str;
}

pub trait OauthError: std::error::Error {}

pub trait OauthErrorAccessTokenStatus: OauthError {
    /// If this is `true` then most likely current service account
    /// key is invalid.
    fn is_access_token_missing_even_if_server_requests_completed(&self) -> bool;
}

#[derive(Debug, Clone)]
pub struct FcmClientBuilder<T: OauthClient> {
    service_account_key_json_path: Option<PathBuf>,
    token_cache_json_path: Option<PathBuf>,
    fcm_request_timeout: Option<Duration>,
    _phantom: std::marker::PhantomData<T>,
}

impl <T: OauthClient> Default for FcmClientBuilder<T> {
    fn default() -> Self {
        Self {
            service_account_key_json_path: None,
            token_cache_json_path: None,
            fcm_request_timeout: None,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl <T: OauthClient> FcmClientBuilder<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set path to the service account key JSON file. Default is to use
    /// path from the `GOOGLE_APPLICATION_CREDENTIALS` environment variable
    /// (which can be also located in `.env` file).
    pub fn service_account_key_json_path(mut self, service_account_key_json_path: impl AsRef<Path>) -> Self {
        self.service_account_key_json_path = Some(service_account_key_json_path.as_ref().to_path_buf());
        self
    }

    /// Set timeout for FCM requests. Default is no timeout.
    ///
    /// Google recommends at least 10 minute timeout for FCM requests.
    /// <https://firebase.google.com/docs/cloud-messaging/scale-fcm#timeouts>
    pub fn fcm_request_timeout(mut self, fcm_request_timeout: Duration) -> Self {
        self.fcm_request_timeout = Some(fcm_request_timeout);
        self
    }
}

#[cfg(feature = "gauth")]
impl FcmClientBuilder<oauth_gauth::Gauth> {
    pub async fn build(self) -> Result<FcmClient<oauth_gauth::Gauth>, FcmClientError<<oauth_gauth::Gauth as OauthClient>::Error>> {
        Ok(FcmClient {
            internal_client: FcmClientInternal::new_from_builder(self).await?,
        })
    }
}

#[cfg(feature = "yup-oauth2")]
impl FcmClientBuilder<oauth_yup_oauth2::YupOauth2> {
    /// Set path to the token cache JSON file. Default is no token cache JSON file.
    pub fn token_cache_json_path(mut self, token_cache_json_path: impl AsRef<Path>) -> Self {
        self.token_cache_json_path = Some(token_cache_json_path.as_ref().to_path_buf());
        self
    }

    pub async fn build(self) -> Result<FcmClient<oauth_yup_oauth2::YupOauth2>, FcmClientError<<oauth_yup_oauth2::YupOauth2 as OauthClient>::Error>> {
        Ok(FcmClient {
            internal_client: FcmClientInternal::new_from_builder(self).await?,
        })
    }
}

/// An async client for sending the notification payload.
pub struct FcmClient<T: OauthClient = DefaultOauthClient> {
    internal_client: FcmClientInternal<T>,
}

impl FcmClient<DefaultOauthClient> {
    pub fn builder() -> FcmClientBuilder<DefaultOauthClient> {
        FcmClientBuilder::new()
    }
}

#[cfg(feature = "gauth")]
impl FcmClient<oauth_gauth::Gauth> {
    pub async fn send(&self, message: Message) -> Result<FcmResponse, FcmClientError<<oauth_gauth::Gauth as OauthClient>::Error>> {
        self.internal_client.send(message).await
    }
}

#[cfg(feature = "yup-oauth2")]
impl FcmClient<oauth_yup_oauth2::YupOauth2> {
    pub async fn send(&self, message: Message) -> Result<FcmResponse, FcmClientError<<oauth_yup_oauth2::YupOauth2 as OauthClient>::Error>> {
        self.internal_client.send(message).await
    }
}
