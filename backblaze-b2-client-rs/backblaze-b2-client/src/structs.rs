use backblaze_b2_client_macros::{self, b2_basic_body_init, IntoHeaderMap};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{
    de::{self, Unexpected, Visitor},
    ser::SerializeStruct,
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::num::NonZeroU64;
use std::{
    collections::HashMap,
    error::Error,
    fmt,
    num::{NonZeroU16, NonZeroU32},
    str::FromStr,
};
use strum_macros::{Display, EnumString};
use typed_builder::TypedBuilder;
#[derive(Clone, Deserialize, Debug)]
// #[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
#[serde(rename_all = "camelCase")]
pub struct B2AuthData {
    pub account_id: String,
    pub authorization_token: String,
    pub allowed: B2AuthDataAllowed,
    pub api_url: String,
    pub download_url: String,
    pub recommended_part_size: NonZeroU64,
    pub absolute_minimum_part_size: NonZeroU64,
    pub s3_api_url: String,
}
pub struct B2Client {
    pub reqwest_client: reqwest::Client,
    pub auth_data: B2AuthData,
}

#[derive(Debug, EnumString, Display, Clone, Copy, PartialEq)]
#[strum(serialize_all = "camelCase")]
pub enum B2KeyCapabilities {
    ListKeys,
    WriteKeys,
    DeleteKeys,
    ListBuckets,
    ListAllBucketNames,
    ReadBuckets,
    WriteBuckets,
    DeleteBuckets,
    ReadBucketRetentions,
    WriteBucketRetentions,
    ReadBucketEncryption,
    WriteBucketEncryption,
    ListFiles,
    ReadFiles,
    ShareFiles,
    WriteFiles,
    DeleteFiles,
    ReadFileLegalHolds,
    WriteFileLegalHolds,
    ReadFileRetentions,
    WriteFileRetentions,
    BypassGovernance,
    ReadBucketReplications,
    WriteBucketReplications,
}

struct B2KeyCapabilitiesVisitor;

impl<'de> Visitor<'de> for B2KeyCapabilitiesVisitor {
    type Value = B2KeyCapabilities;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("A valid B2 bucket permission.")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match B2KeyCapabilities::from_str(s) {
            Ok(permission) => Ok(permission),
            Err(error) => Err(de::Error::invalid_value(
                Unexpected::Str(&error.to_string()),
                &self,
            )),
        }
    }
}

#[derive(Debug, EnumString, Display, Clone, PartialEq)]
#[strum(serialize_all = "camelCase")]
pub enum B2Actions {
    Start,
    Upload,
    Hide,
    Folder,
}

impl Serialize for B2Actions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

struct B2ActionsVisitor;

impl<'de> Visitor<'de> for B2ActionsVisitor {
    type Value = B2Actions;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("A valid B2 bucket permission.")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match B2Actions::from_str(s) {
            Ok(permission) => Ok(permission),
            Err(error) => Err(de::Error::invalid_value(
                Unexpected::Str(&error.to_string()),
                &self,
            )),
        }
    }
}

impl<'de> Deserialize<'de> for B2Actions {
    fn deserialize<D>(deserializer: D) -> Result<B2Actions, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(B2ActionsVisitor)
    }
}

impl<'de> Deserialize<'de> for B2KeyCapabilities {
    fn deserialize<D>(deserializer: D) -> Result<B2KeyCapabilities, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(B2KeyCapabilitiesVisitor)
    }
}

#[derive(Debug, PartialEq, EnumString)]
enum B2Error {
    BadBucketId,
    BadRequest,
    CannotDeleteNonEmptyBucket,
    Unauthorized,
    Unsupported,
    TransactionCapExceeded,
}

impl fmt::Display for B2Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "B2 encountered an error: {}", self.to_string())
    }
}

impl Error for B2Error {}

#[derive(Clone, Deserialize, Debug, Serialize)]
pub enum B2BasicError {
    NotAuthenticated,
    JsonParseError(String),
    RequestError(B2BasicErrorBody),
    RequestSendError(String),
}

impl Error for B2BasicError {}

impl fmt::Display for B2BasicError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "B2 request encountered an error: {}",
            serde_json::to_string(self).unwrap()
        )
    }
}

#[derive(Clone, Deserialize, Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum B2ReplicationStatus {
    Pending,
    Completed,
    Failed,
    Replica,
}

#[derive(Clone, Deserialize, Debug, Serialize)]
pub struct B2BasicErrorBody {
    pub status: NonZeroU16,
    pub code: String,
    pub message: String,
}

impl fmt::Display for B2BasicErrorBody {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", serde_json::to_string(self).unwrap())
    }
}

impl Error for B2BasicErrorBody {}

#[derive(Clone, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct B2AuthDataAllowed {
    pub capabilities: Vec<B2KeyCapabilities>,
    pub bucket_id: Option<String>,
    pub bucket_name: Option<String>,
    pub name_prefix: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct B2LifeCycleRules<'a> {
    pub days_from_hiding_to_deleting: Option<u32>,
    pub days_from_uploading_to_hiding: Option<u32>,
    pub file_name_prefix: &'a str,
}

#[derive(Clone, Deserialize, Debug, Serialize)]
// #[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
#[serde(rename_all = "camelCase")]
pub struct B2ServerSideEncryption {
    pub mode: Option<String>,
    pub algorithm: Option<String>,
}

#[derive(Clone, Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct B2FileRetentionPeriod {
    pub duration: u64,
    pub unit: String,
}

#[derive(Clone, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct B2FileRetention {
    pub mode: Option<String>,
    pub period: Option<B2FileRetentionPeriod>,
}

impl Serialize for B2FileRetention {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let num_of_fields = if self.mode.is_some() { 2 } else { 1 };
        let mut retention = serializer.serialize_struct("fileRetention", num_of_fields)?;
        retention.serialize_field("mode", &self.mode)?;

        if self.mode.is_some() {
            retention.serialize_field("period", &self.period)?;
        }

        retention.end()
    }
}

#[derive(Clone, Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct B2ObjectLockValue {
    pub default_retention: B2FileRetention,
    pub is_file_lock_enabled: bool,
}

#[derive(Clone, Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct B2ObjectLock {
    pub is_client_authorized_to_read: bool,
    pub value: Option<B2ObjectLockValue>,
}

#[derive(Clone, Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct B2File {
    pub account_id: String,
    pub action: String,
    pub bucket_id: String,
    pub content_length: u64,
    pub content_sha1: String,
    pub content_md5: Option<String>,
    pub content_type: String,
    pub file_id: String,
    pub file_info: HashMap<String, String>,
    pub file_name: String,
    pub file_retention: B2ObjectLock,
    pub legal_hold: B2ObjectLock,
    pub replication_status: Option<B2ReplicationStatus>,
    pub server_side_encryption: B2ServerSideEncryption,
    pub upload_timestamp: u64,
}

#[derive(Clone, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct B2FilePart {
    pub file_id: String,
    pub part_number: u16,
    pub content_length: u64,
    pub content_sha1: String,
    pub content_md5: Option<String>,
    pub server_side_encryption: B2ServerSideEncryption,
    pub upload_timestamp: u64,
}

#[derive(Clone, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct B2ListFilesResponse {
    pub files: Vec<B2File>,
    pub next_file_name: Option<String>,
}

#[derive(Clone, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct B2GetUploadUrlResponse {
    pub bucket_id: String,
    pub upload_url: String,
    pub authorization_token: String,
}

#[b2_basic_body_init]
pub struct B2ListFilesBody {
    #[builder(default)]
    pub start_file_name: Option<String>,
    #[builder(default, setter(strip_option))]
    pub max_file_count: Option<NonZeroU32>,
    #[builder(default, setter(strip_option))]
    pub prefix: Option<String>,
    #[builder(default, setter(strip_option))]
    pub delimiter: Option<String>,
}

#[b2_basic_body_init]
pub struct B2GetUploadUrlBody {}

#[derive(Clone, Serialize, Debug)]
pub enum B2ServerSideEncryptionAlgorithm {
    AES256,
}

#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum B2FileRetentionMode {
    Governance,
    Compliance,
}

#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum B2FileLegalHold {
    On,
    Off,
}

// #[backblaze_b2_client_macros::impl_into_header_map]
#[derive(Clone, Debug, Serialize, TypedBuilder, IntoHeaderMap)]
pub struct B2UploadFileHeaders {
    #[serde(rename = "Authorization")]
    pub authorization: String,
    #[serde(rename = "X-Bz-File-Name")]
    pub file_name: String,
    #[serde(rename = "Content-Type")]
    pub content_type: String,
    #[serde(rename = "Content-Length")]
    pub content_length: u32,
    #[serde(rename = "X-Bz-Content-Sha1")]
    pub content_sha1: String,
    #[serde(rename = "X-Bz-Info-src_last_modified_millis")]
    #[builder(default, setter(strip_option))]
    pub src_last_modified_millis: Option<u32>,
    #[serde(rename = "X-Bz-Info-b2-content-disposition")]
    #[builder(default, setter(strip_option))]
    pub b2_content_disposition: Option<String>,
    #[serde(rename = "X-Bz-Info-b2-content-language")]
    #[builder(default, setter(strip_option))]
    pub b2_content_language: Option<String>,
    #[serde(rename = "X-Bz-Info-b2-expires")]
    #[builder(default, setter(strip_option))]
    pub b2_expires: Option<String>,
    #[serde(rename = "X-Bz-Info-b2-cache-control")]
    #[builder(default, setter(strip_option))]
    pub b2_cache_control: Option<String>,
    #[serde(rename = "X-Bz-Info-b2-content-encoding")]
    #[builder(default, setter(strip_option))]
    pub b2_content_encoding: Option<String>,
    #[serde(rename = "X-Bz-Custom-Upload-Timestamp")]
    #[builder(default, setter(strip_option))]
    pub custom_upload_timestamp: Option<u32>,
    #[serde(rename = "X-Bz-File-Legal-Hold")]
    #[builder(default, setter(strip_option))]
    pub legal_hold: Option<B2FileLegalHold>,
    #[serde(rename = "X-Bz-File-Retention-Mode")]
    #[builder(default, setter(strip_option))]
    pub retention_mode: Option<B2FileRetentionMode>,
    #[serde(rename = "X-Bz-File-Retention-Retain-Until-Timestamp")]
    #[builder(default, setter(strip_option))]
    pub retention_retain_until_timestamp: Option<u32>,
    #[serde(rename = "X-Bz-Server-Side-Encryption")]
    #[builder(default, setter(strip_option))]
    pub server_side_encryption: Option<B2ServerSideEncryptionAlgorithm>,
    #[serde(rename = "X-Bz-Server-Side-Encryption-Customer-Algorithm")]
    #[builder(default, setter(strip_option))]
    pub server_side_encryption_customer_algorithm: Option<B2ServerSideEncryptionAlgorithm>,
    #[serde(rename = "X-Bz-Server-Side-Encryption-Customer-Key")]
    #[builder(default, setter(strip_option))]
    pub server_side_encryption_customer_key: Option<String>,
    #[serde(rename = "X-Bz-Server-Side-Encryption-Customer-Key-Md5")]
    #[builder(default, setter(strip_option))]
    pub server_side_encryption_customer_key_md5: Option<String>,
}

#[derive(Clone, Debug, Serialize, TypedBuilder, IntoHeaderMap)]
pub struct B2UploadPartHeaders {
    #[serde(rename = "Authorization")]
    pub authorization: String,
    #[serde(rename = "X-Bz-Part-Number")]
    pub part_number: u16,
    #[serde(rename = "Content-Length")]
    pub content_length: u32,
    #[serde(rename = "X-Bz-Content-Sha1")]
    pub content_sha1: String,
    #[serde(rename = "X-Bz-Server-Side-Encryption-Customer-Algorithm")]
    #[builder(default, setter(strip_option))]
    pub server_side_encryption_customer_algorithm: Option<B2ServerSideEncryptionAlgorithm>,
    #[serde(rename = "X-Bz-Server-Side-Encryption-Customer-Key")]
    #[builder(default, setter(strip_option))]
    pub server_side_encryption_customer_key: Option<String>,
    #[serde(rename = "X-Bz-Server-Side-Encryption-Customer-Key-Md5")]
    #[builder(default, setter(strip_option))]
    pub server_side_encryption_customer_key_md5: Option<String>,
}

#[derive(Clone, Debug, Serialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct B2StartLargeFileUploadBody {
    bucket_id: String,
    file_name: String,
    content_type: String,
    #[builder(default, setter(strip_option))]
    custom_upload_timestamp: Option<String>,
    #[builder(default, setter(strip_option))]
    file_retention: Option<B2FileRetention>,
    #[builder(default, setter(strip_option))]
    legal_hold: Option<B2FileLegalHold>,
    #[builder(default, setter(strip_option))]
    server_side_encryption: Option<B2ServerSideEncryption>,
    #[builder(default)]
    file_info: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2GetUploadPartUrlResponse {
    pub file_id: String,
    pub upload_url: String,
    pub authorization_token: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2StartLargeFileUploadResponse {
    pub account_id: String,
    pub action: B2Actions,
    pub bucket_id: String,
    pub content_length: i128,
    pub content_sha1: String,
    pub content_md5: Option<String>,
    pub content_type: String,
    pub file_id: String,
    pub file_info: HashMap<String, String>,
    pub file_name: String,
    pub file_retention: B2ObjectLock,
    pub legal_hold: B2ObjectLock,
    pub replication_status: Option<B2ReplicationStatus>,
    pub service_side_encryption: Option<B2ServerSideEncryption>,
    pub upload_timestamp: u64,
}

#[derive(Clone, Debug, Serialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct B2UpdateFileRetentionBody {
    pub file_name: String,
    pub file_id: String,
    pub file_retention: B2FileRetention,
    #[builder(default, setter(strip_option))]
    pub bypass_governance: Option<bool>,
}

#[derive(Clone, Debug, Serialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct B2FinishLargeFileBody {
    file_id: String,
    part_sha1_array: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2UpdateFileRetentionResponse {
    pub file_name: String,
    pub file_id: String,
    pub file_retention: B2FileRetention,
}
