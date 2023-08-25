use std::hash::Hash;

pub use common::{BackupRequest, SignedBackupRequest};
use config::MintClientConfig;
use fedimint_core::core::{Decoder, ModuleInstanceId, ModuleKind};
use fedimint_core::encoding::{Decodable, Encodable};
use fedimint_core::module::{CommonModuleInit, ModuleCommon, ModuleConsensusVersion};
use fedimint_core::tiered::InvalidAmountTierError;
use fedimint_core::{plugin_types_trait_impl_common, Amount, OutPoint, PeerId, TieredMulti};
use impl_tools::autoimpl;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::error;

pub mod config;

pub mod common;
pub mod db;

pub const KIND: ModuleKind = ModuleKind::from_static_str("mint");
const CONSENSUS_VERSION: ModuleConsensusVersion = ModuleConsensusVersion(0);

/// By default, the maximum notes per denomination when change-making for users
pub const DEFAULT_MAX_NOTES_PER_DENOMINATION: u16 = 3;

/// Data structures taking into account different amount tiers

/// A consenus item from one of the federation members contributing partials
/// signatures to blind nonces submitted in it
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Encodable, Decodable)]
pub struct MintConsensusItem {
    /// Reference to a Federation Transaction containing an [`MintOutput`] with
    /// `BlindNonce`s the signatures` are for
    pub out_point: OutPoint,
    /// (Partial) signatures
    pub signatures: MintOutputSignatureShare,
}

// FIXME: optimize out blinded msg by making the mint remember it
/// Blind signature share from one Federation peer for a single [`MintOutput`]
#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize, Encodable, Decodable)]
pub struct MintOutputSignatureShare(
    pub TieredMulti<(tbs::BlindedMessage, tbs::BlindedSignatureShare)>,
);

/// Result of Federation members confirming [`MintOutput`] by contributing
/// partial signatures via [`MintConsensusItem`]
///
/// A set of full blinded signatures.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize, Encodable, Decodable)]
pub struct MintOutputBlindSignatures(pub TieredMulti<tbs::BlindedSignature>);

/// An verifiable one time use IOU from the mint.
///
/// Digital version of a "note of deposit" in a free-banking era.
///
/// Consist of a user-generated nonce and a threshold signature over it
/// generated by the federated mint (while in a [`BlindNonce`] form).
///
/// As things are right now the denomination of each note is determined by the
/// federation keys that signed over it, and needs to be tracked outside of this
/// type.
///
/// In this form it can only be validated, not spent since for that the
/// corresponding secret spend key is required.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Deserialize, Serialize, Encodable, Decodable)]
pub struct Note(pub Nonce, pub tbs::Signature);

/// Unique ID of a mint note.
///
/// User-generated, random or otherwise unpredictably generated
/// (deterministically derived).
///
/// Internally a MuSig pub key so that transactions can be signed when being
/// spent.
#[derive(
    Debug,
    Copy,
    Clone,
    Eq,
    PartialEq,
    PartialOrd,
    Ord,
    Hash,
    Deserialize,
    Serialize,
    Encodable,
    Decodable,
)]
pub struct Nonce(pub secp256k1_zkp::XOnlyPublicKey);

/// [`Nonce`] but blinded by the user key
///
/// Blinding prevents the Mint from being able to link the transaction spending
/// [`Note`]s as an `Input`s of `Transaction` with new [`Note`]s being created
/// in its `Output`s.
///
/// By signing it, the mint commits to the underlying (unblinded) [`Nonce`] as
/// valid (until eventually spent).
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Deserialize, Serialize, Encodable, Decodable)]
pub struct BlindNonce(pub tbs::BlindedMessage);

#[derive(Debug)]
pub struct MintCommonGen;

impl CommonModuleInit for MintCommonGen {
    const CONSENSUS_VERSION: ModuleConsensusVersion = CONSENSUS_VERSION;
    const KIND: ModuleKind = KIND;

    type ClientConfig = MintClientConfig;

    fn decoder() -> Decoder {
        MintModuleTypes::decoder_builder().build()
    }
}

#[autoimpl(Deref, DerefMut using self.0)]
#[derive(
    Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize, Encodable, Decodable, Default,
)]
pub struct MintInput(pub TieredMulti<Note>);

impl std::fmt::Display for MintInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mint Notes {}", self.0.total_amount())
    }
}

#[autoimpl(Deref, DerefMut using self.0)]
#[derive(
    Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize, Encodable, Decodable, Default,
)]
pub struct MintOutput(pub TieredMulti<BlindNonce>);

impl std::fmt::Display for MintOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mint Notes {}", self.0.total_amount())
    }
}

#[autoimpl(Deref, DerefMut using self.0)]
#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize, Encodable, Decodable)]
pub struct MintOutputOutcome(pub Option<MintOutputBlindSignatures>);

impl std::fmt::Display for MintOutputOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(sigs) => {
                write!(f, "Minted notes of value {}", sigs.0.total_amount())
            }
            None => {
                write!(f, "To-be-minted notes")
            }
        }
    }
}

impl std::fmt::Display for MintConsensusItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Mint Blind Signature Shares worth {} for {}",
            self.signatures.0.total_amount(),
            self.out_point
        )
    }
}

pub struct MintModuleTypes;

impl Note {
    /// Verify the note's validity under a mit key `pk`
    pub fn verify(&self, pk: tbs::AggregatePublicKey) -> bool {
        tbs::verify(self.0.to_message(), self.1, pk)
    }

    /// Access the nonce as the public key to the spend key
    pub fn spend_key(&self) -> &secp256k1_zkp::XOnlyPublicKey {
        &self.0 .0
    }
}

impl Nonce {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![];
        bincode::serialize_into(&mut bytes, &self.0).unwrap();
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        // FIXME: handle errors or the client can be crashed
        bincode::deserialize(bytes).unwrap()
    }

    pub fn to_message(&self) -> tbs::Message {
        tbs::Message::from_bytes(&self.0.serialize()[..])
    }
}

impl From<MintOutput> for TieredMulti<BlindNonce> {
    fn from(sig_req: MintOutput) -> Self {
        sig_req.0
    }
}

impl Extend<(Amount, BlindNonce)> for MintOutput {
    fn extend<T: IntoIterator<Item = (Amount, BlindNonce)>>(&mut self, iter: T) {
        self.0.extend(iter)
    }
}

plugin_types_trait_impl_common!(
    MintModuleTypes,
    MintClientConfig,
    MintInput,
    MintOutput,
    MintOutputOutcome,
    MintConsensusItem
);

/// Represents an array of mint indexes that delivered faulty shares
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct MintShareErrors(pub Vec<(PeerId, PeerErrorType)>);

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum PeerErrorType {
    InvalidSignature,
    DifferentStructureSigShare,
    DifferentNonce,
    InvalidAmountTier,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Error)]
pub enum CombineError {
    #[error("Too few shares to begin the combination: got {0:?} need {1}")]
    TooFewShares(Vec<PeerId>, usize),
    #[error(
        "Too few valid shares, only {0} of {1} (required minimum {2}) provided shares were valid"
    )]
    TooFewValidShares(usize, usize, usize),
    #[error("We could not find our own contribution in the provided shares, so we have no validation reference")]
    NoOwnContribution,
    #[error("Peer {0} contributed {1} shares, 1 expected")]
    MultiplePeerContributions(PeerId, usize),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Error)]
pub enum MintError {
    #[error("One of the supplied notes had an invalid mint signature")]
    InvalidNote,
    #[error("Insufficient note value: reissuing {0} but only got {1} in notes")]
    TooFewNotes(Amount, Amount),
    #[error("One of the supplied notes was already spent previously")]
    SpentCoin,
    #[error("One of the notes had an invalid amount not issued by the mint: {0:?}")]
    InvalidAmountTier(Amount),
    #[error("One of the notes had an invalid signature")]
    InvalidSignature,
    #[error("Exceeded maximum notes per denomination {0}, found {1}")]
    ExceededMaxNotes(u16, usize),
}

impl From<InvalidAmountTierError> for MintError {
    fn from(e: InvalidAmountTierError) -> Self {
        MintError::InvalidAmountTier(e.0)
    }
}
