//! Signing block data structures

use serde::{Deserialize, Serialize};

/// Complete signing block inserted into ZIP (before Central Directory)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningBlock {
    pub signers: Vec<Signer>,
}

/// Signer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signer {
    pub certificates: Vec<Certificate>,
    pub digest_algorithm: DigestAlgorithm,
    pub digests: Vec<SectionDigest>,
    pub signature: Vec<u8>,
    pub signed_attrs: SignedAttributes,
}

/// X.509 Certificate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certificate {
    pub data: Vec<u8>,
    pub identity: SignerIdentity,
}

/// Signer identity type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignerIdentity {
    Developer,
    Platform,
    CaIssued,
}

/// Digest algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DigestAlgorithm {
    Sha256,
}

/// Section digest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionDigest {
    pub section_name: String,
    pub digest: Vec<u8>,
}

/// Signed attributes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedAttributes {
    pub signing_time: String,
}
