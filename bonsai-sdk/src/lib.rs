#![doc = include_str!("../README.md")]
#![deny(missing_docs)]

use std::{fs::File, path::Path};

/// Bonsai SDK for interacting with the REST api
use anyhow::{bail, Context, Result};
use reqwest::{blocking::Client as BlockingClient, header};

use self::responses::{CreateSessRes, ProofReq, SessionStatusRes, UploadRes};

/// Collection of serialization object for the REST api
pub mod responses {
    use serde::{Deserialize, Serialize};

    /// Response of a upload request
    #[derive(Deserialize, Serialize)]
    pub struct UploadRes {
        /// Presigned URL to be supplied to a PUT request
        pub url: String,
        /// Generated UUID for this input
        pub uuid: String,
    }

    /// Session creation response
    #[derive(Deserialize, Serialize)]
    pub struct CreateSessRes {
        /// Generated UUID for the session
        pub uuid: String,
    }

    /// Proof Request object to create Session
    #[derive(Deserialize, Serialize)]
    pub struct ProofReq {
        /// Image UUID
        pub img: String,
        /// Input UUID
        pub input: String,
    }

    /// Session Status response
    #[derive(Deserialize, Serialize)]
    pub struct SessionStatusRes {
        /// Current status
        ///
        /// values: [RUNNING | SUCCEEDED | FAILED | TIMED_OUT | ABORTED |
        /// SUCCEEDED]
        pub status: String,
        /// Final receipt download URL
        ///
        /// If the status == 'SUCCEEDED' then this should be present
        pub receipt_url: Option<String>,
    }
}

/// Proof Session representation
pub struct SessionId {
    /// Session UUID
    pub uuid: String,
}

impl SessionId {
    /// Construct a [SessionId] from a UUID [String]
    pub fn new(uuid: String) -> Self {
        Self { uuid }
    }

    /// Retries the current status of the Session
    pub fn status(&self, client: &Client) -> Result<SessionStatusRes> {
        let url = format!("{}/sessions/status/{}", client.url, self.uuid);
        let res = client
            .client
            .get(url)
            .send()
            .context("Failed to GEt session status")?;

        if !res.status().is_success() {
            let body = res.text()?;
            bail!("Request failed - server error: '{body}'");
        }
        res.json::<SessionStatusRes>()
            .context("Failed to deserialize Session status result")
    }
}

/// Represents a client of the REST api
pub struct Client {
    pub(crate) url: String,
    pub(crate) client: BlockingClient,
}

/// Creates a [reqwest::Client] for internal connection pooling
fn construct_req_client(api_key: &str) -> Result<BlockingClient> {
    let mut headers = header::HeaderMap::new();
    headers.insert("x-api-key", header::HeaderValue::from_str(api_key)?);

    BlockingClient::builder()
        .default_headers(headers)
        .build()
        .context("Failed to build reqwest client")
}

impl Client {
    /// Construct a [Client] from env var
    ///
    /// Uses the BONSAI_ENDPOINT environment variables to construct a client
    /// The BONSAI_ENDPOINT string packs both the API Url and API_KEY into the
    /// same string with the following format:
    /// <api_url>|<api_key>
    pub fn from_env() -> Result<Self> {
        let bonsai_endpoint =
            std::env::var("BONSAI_ENDPOINT").context("Missing BONSAI_ENDPOINT env var")?;

        let parts = bonsai_endpoint.split('|').collect::<Vec<&str>>();
        if parts.len() != 2 {
            bail!("Invalid BONSAI_ENDPOINT URL, must be in format: '<api_url>|<api_key>'");
        }

        let url = parts[0].to_string();
        let key = parts[1].to_string();

        let client = construct_req_client(&key)?;

        Ok(Self { url, client })
    }

    /// Construct a [Client] from url + api key strings
    pub fn from_parts(url: String, key: String) -> Result<Self> {
        let client = construct_req_client(&key)?;
        Ok(Self { url, client })
    }

    /// Fetch a upload presigned url for a given route
    fn get_upload_url(&self, route: &str) -> Result<UploadRes> {
        let res = self
            .client
            .get(format!("{}/{}/upload", self.url, route))
            .send()
            .context("Failed to fetch upload location")?;

        if !res.status().is_success() {
            let body = res.text()?;
            bail!("Request failed - server error: '{body}'");
        }

        res.json::<UploadRes>()
            .context("Failed to deserialize upload response")
    }

    /// Upload body to a given URL
    fn put_data<T: Into<reqwest::blocking::Body>>(&self, url: &str, body: T) -> Result<()> {
        let res = self
            .client
            .put(url)
            .body(body)
            .send()
            .context("Failed to PUT data to destination")?;
        if !res.status().is_success() {
            bail!("Failed to PUT to provided URL");
        }

        Ok(())
    }

    // - /images

    /// Upload a image buffer to the /images/ route
    pub fn upload_img(&self, buf: Vec<u8>) -> Result<String> {
        let upload_data = self.get_upload_url("images")?;
        self.put_data(&upload_data.url, buf)?;
        Ok(upload_data.uuid)
    }

    /// Upload a image file to the /images/ route
    pub fn upload_img_file(&self, path: &Path) -> Result<String> {
        let upload_data = self.get_upload_url("images")?;

        let fd = File::open(path).context("Unable to open supplied image file")?;
        self.put_data(&upload_data.url, fd)?;

        Ok(upload_data.uuid)
    }

    // - /inputs

    /// Upload a input buffer to the /inputs/ route
    pub fn upload_input(&self, buf: Vec<u8>) -> Result<String> {
        let upload_data = self.get_upload_url("inputs")?;
        self.put_data(&upload_data.url, buf)?;
        Ok(upload_data.uuid)
    }

    /// Upload a input file to the /inputs/ route
    pub fn upload_input_file(&self, path: &Path) -> Result<String> {
        let upload_data = self.get_upload_url("inputs")?;

        let fd = File::open(path).context("Unable to open supplied image file")?;
        self.put_data(&upload_data.url, fd)?;

        Ok(upload_data.uuid)
    }

    // - /sessions

    /// Create a new proof request Session
    ///
    /// Supply the img_id and input_id created from uploading those files in
    /// previous steps
    pub fn create_session(&self, img_id: String, input_id: String) -> Result<SessionId> {
        let url = format!("{}/sessions/create", self.url);

        let req = ProofReq {
            img: img_id,
            input: input_id,
        };

        let res = self
            .client
            .post(url)
            .json(&req)
            .send()
            .context("Failed to submit session/create POST request")?;

        if !res.status().is_success() {
            let body = res.text()?;
            bail!("Request failed - server error: '{body}'");
        }

        let res: CreateSessRes = res
            .json()
            .context("Failed to deserialize Session status result")?;

        Ok(SessionId::new(res.uuid))
    }

    // Utilities

    /// Download a given url to a buffer
    ///
    /// Useful to download a [SessionId] receipt_url
    pub fn download(&self, url: &str) -> Result<Vec<u8>> {
        let data = self
            .client
            .get(url)
            .send()
            .context("Failed to download url to buffer")?
            .bytes()
            .context("Failed to get raw bytes from download")?;

        Ok(data.into())
    }
}
