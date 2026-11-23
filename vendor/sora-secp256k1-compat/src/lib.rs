//! SORA Nexus protocol compatibility for legacy secp256k1 byte encodings.
//!
//! All curve arithmetic, secret-key handling, deterministic signing, verification
//! and recovery use maintained RustCrypto k256. This deliberately implements only
//! the byte-oriented API needed by this workspace, not the retired library's
//! arithmetic internals. Legacy overflowing scalars and hybrid public keys remain
//! accepted where existing runtime host-function versions require them.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

use k256::{
    ecdsa::{signature::hazmat::PrehashVerifier, SigningKey, VerifyingKey},
    elliptic_curve::{ops::Reduce, sec1::ToSec1Point, PrimeField},
    FieldBytes, Scalar,
};

/// Wire-format sizes used by the existing bridge and runtime host functions.
pub mod util {
    pub const SIGNATURE_SIZE: usize = 64;
    pub const MESSAGE_SIZE: usize = 32;
    pub const SECRET_KEY_SIZE: usize = 32;
    pub const FULL_PUBLIC_KEY_SIZE: usize = 65;
    pub const COMPRESSED_PUBLIC_KEY_SIZE: usize = 33;
    pub const RAW_PUBLIC_KEY_SIZE: usize = 64;
}

/// Parsing and verification errors retained at the byte-oriented API boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidSignature,
    InvalidPublicKey,
    InvalidSecretKey,
    InvalidRecoveryId,
    InvalidInputLength,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::InvalidSignature => "Invalid signature",
            Self::InvalidPublicKey => "Invalid public key",
            Self::InvalidSecretKey => "Invalid secret key",
            Self::InvalidRecoveryId => "Invalid recovery ID",
            Self::InvalidInputLength => "Invalid input length",
        })
    }
}

impl core::error::Error for Error {}

fn reduce(bytes: &[u8; 32]) -> [u8; 32] {
    <Scalar as Reduce<FieldBytes>>::reduce(&(*bytes).into())
        .to_bytes()
        .into()
}

/// Public key, validated by k256 on construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicKey(k256::PublicKey);

/// Accepted public-key wire representations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicKeyFormat {
    Compressed,
    Full,
    Raw,
}

impl PublicKey {
    pub fn from_secret_key(secret: &SecretKey) -> Self {
        Self(secret.0.public_key())
    }

    pub fn parse_slice(bytes: &[u8], format: Option<PublicKeyFormat>) -> Result<Self, Error> {
        let format = match (bytes.len(), format) {
            (33, None | Some(PublicKeyFormat::Compressed)) => PublicKeyFormat::Compressed,
            (65, None | Some(PublicKeyFormat::Full)) => PublicKeyFormat::Full,
            (64, None | Some(PublicKeyFormat::Raw)) => PublicKeyFormat::Raw,
            _ => return Err(Error::InvalidInputLength),
        };
        match format {
            PublicKeyFormat::Compressed => Self::parse_compressed(bytes.try_into().unwrap()),
            PublicKeyFormat::Full => Self::parse(bytes.try_into().unwrap()),
            PublicKeyFormat::Raw => {
                let mut full = [0u8; 65];
                full[0] = 4;
                full[1..].copy_from_slice(bytes);
                Self::parse(&full)
            }
        }
    }

    pub fn parse(bytes: &[u8; 65]) -> Result<Self, Error> {
        let mut encoded = *bytes;
        match encoded[0] {
            4 => {}
            // Historical SEC1 hybrid form: the tag must agree with Y parity.
            6 | 7 if encoded[0] & 1 == encoded[64] & 1 => encoded[0] = 4,
            _ => return Err(Error::InvalidPublicKey),
        }
        k256::PublicKey::from_sec1_bytes(&encoded)
            .map(Self)
            .map_err(|_| Error::InvalidPublicKey)
    }

    pub fn parse_compressed(bytes: &[u8; 33]) -> Result<Self, Error> {
        k256::PublicKey::from_sec1_bytes(bytes)
            .map(Self)
            .map_err(|_| Error::InvalidPublicKey)
    }

    pub fn serialize(&self) -> [u8; 65] {
        self.0.to_sec1_point(false).as_bytes().try_into().unwrap()
    }

    pub fn serialize_compressed(&self) -> [u8; 33] {
        self.0.to_sec1_point(true).as_bytes().try_into().unwrap()
    }
}

/// Secret key owned by k256 and zeroized when dropped.
#[derive(Clone)]
pub struct SecretKey(k256::SecretKey);

impl SecretKey {
    pub fn parse(bytes: &[u8; 32]) -> Result<Self, Error> {
        k256::SecretKey::from_bytes(&(*bytes).into())
            .map(Self)
            .map_err(|_| Error::InvalidSecretKey)
    }

    pub fn parse_slice(bytes: &[u8]) -> Result<Self, Error> {
        Self::parse(bytes.try_into().map_err(|_| Error::InvalidInputLength)?)
    }

    pub fn serialize(&self) -> [u8; 32] {
        self.0.to_bytes().into()
    }
}

impl core::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("SecretKey([REDACTED])")
    }
}

/// Signature scalars in canonical 32-byte encoding, including zero for legacy parsing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Signature([u8; 64]);

impl Signature {
    pub fn parse_overflowing(bytes: &[u8; 64]) -> Self {
        let mut result = [0; 64];
        result[..32].copy_from_slice(&reduce(bytes[..32].try_into().unwrap()));
        result[32..].copy_from_slice(&reduce(bytes[32..].try_into().unwrap()));
        Self(result)
    }

    pub fn parse_overflowing_slice(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self::parse_overflowing(
            bytes.try_into().map_err(|_| Error::InvalidInputLength)?,
        ))
    }

    pub fn parse_standard(bytes: &[u8; 64]) -> Result<Self, Error> {
        // Legacy parsing accepts zero scalars; verification/recovery reject them.
        for chunk in bytes.chunks_exact(32) {
            let scalar: [u8; 32] = chunk.try_into().unwrap();
            if bool::from(Scalar::from_repr(scalar.into()).is_none()) {
                return Err(Error::InvalidSignature);
            }
        }
        Ok(Self(*bytes))
    }

    pub fn parse_standard_slice(bytes: &[u8]) -> Result<Self, Error> {
        Self::parse_standard(bytes.try_into().map_err(|_| Error::InvalidInputLength)?)
    }

    pub fn serialize(&self) -> [u8; 64] {
        self.0
    }

    fn to_k256(self) -> Result<k256::ecdsa::Signature, Error> {
        k256::ecdsa::Signature::from_slice(&self.0).map_err(|_| Error::InvalidSignature)
    }
}

/// Legacy messages are reduced modulo the curve order before signing/recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Message([u8; 32]);

impl Message {
    pub fn parse(bytes: &[u8; 32]) -> Self {
        Self(reduce(bytes))
    }

    pub fn parse_slice(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self::parse(
            bytes.try_into().map_err(|_| Error::InvalidInputLength)?,
        ))
    }

    pub fn serialize(&self) -> [u8; 32] {
        self.0
    }
}

/// Recovery identifiers are the existing two-bit values, not Ethereum chain IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryId(k256::ecdsa::RecoveryId);

impl RecoveryId {
    pub fn parse(value: u8) -> Result<Self, Error> {
        k256::ecdsa::RecoveryId::from_byte(value)
            .map(Self)
            .ok_or(Error::InvalidRecoveryId)
    }

    pub fn parse_rpc(value: u8) -> Result<Self, Error> {
        value
            .checked_sub(27)
            .ok_or(Error::InvalidRecoveryId)
            .and_then(Self::parse)
    }

    pub fn serialize(&self) -> u8 {
        self.0.to_byte()
    }
}

impl From<RecoveryId> for u8 {
    fn from(id: RecoveryId) -> Self {
        id.serialize()
    }
}

/// Deterministic RFC6979 ECDSA signing, with the existing low-S/recovery-ID output.
pub fn sign(message: &Message, secret: &SecretKey) -> (Signature, RecoveryId) {
    let (signature, id) = SigningKey::from(&secret.0).sign_prehash_recoverable(&message.0);
    (Signature(signature.to_bytes().into()), RecoveryId(id))
}

/// Verify both historical low-S and high-S signatures.
pub fn verify(message: &Message, signature: &Signature, public: &PublicKey) -> bool {
    let Ok(signature) = signature.to_k256() else {
        return false;
    };
    VerifyingKey::from(public.0)
        .verify_prehash(&message.0, &signature.normalize_s())
        .is_ok()
}

pub fn recover(
    message: &Message,
    signature: &Signature,
    id: &RecoveryId,
) -> Result<PublicKey, Error> {
    VerifyingKey::recover_from_prehash(&message.0, &signature.to_k256()?, id.0)
        .map(|key| PublicKey(key.into()))
        .map_err(|_| Error::InvalidSignature)
}
