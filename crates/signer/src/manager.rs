use std::collections::HashMap;

use alloy::rpc::types::beacon::{BlsPublicKey, BlsSignature};
use blst::min_pk::SecretKey as BlsSecretKey;
use cb_common::{
    commit::request::{ProxyDelegation, SignedProxyDelegation},
    signer::{
        EcdsaSecretKey, GenericProxySigner, GenericPubkey, ProxySigner, PubKey, SecretKey, Signer,
    },
    types::{Chain, ModuleId},
};
use tree_hash::TreeHash;

use crate::error::SignerModuleError;

#[derive(Default)]
struct ProxySigners {
    bls_signers: HashMap<PubKey<BlsSecretKey>, ProxySigner<BlsSecretKey>>,
    ecdsa_signers: HashMap<PubKey<EcdsaSecretKey>, ProxySigner<EcdsaSecretKey>>,
}

impl<'a> ProxySigners {
    pub fn get(&self, key: &GenericPubkey) -> Option<GenericProxySigner> {
        match key {
            GenericPubkey::Bls(bls_pubkey) => {
                let proxy_signer = self.get_proxy_signer(bls_pubkey)?;
                Some(GenericProxySigner::Bls(proxy_signer.clone()))
            }
            GenericPubkey::Ecdsa(ecdsa_pubkey) => {
                let proxy_signer = self.get_proxy_signer(ecdsa_pubkey)?;
                Some(GenericProxySigner::Ecdsa(proxy_signer.clone()))
            }
        }
    }

    pub fn add(&mut self, proxy: GenericProxySigner) {
        match proxy {
            GenericProxySigner::Bls(bls_proxy) => {
                self.bls_signers.insert(bls_proxy.pubkey(), bls_proxy);
            }
            GenericProxySigner::Ecdsa(ecdsa_proxy) => {
                self.ecdsa_signers.insert(ecdsa_proxy.pubkey(), ecdsa_proxy);
            }
        }
    }

    pub fn find_pubkey(&'a self, pubkey: &[u8]) -> Option<GenericPubkey> {
        fn find_typed<'a, T>(
            keys: impl IntoIterator<Item = &'a PubKey<T>>,
            pubkey: &[u8],
        ) -> Option<GenericPubkey>
        where
            T: SecretKey,
            PubKey<T>: 'a + Into<GenericPubkey>,
        {
            keys.into_iter().find(|x| x.as_ref() == pubkey).cloned().map(Into::into)
        }

        find_typed::<BlsSecretKey>(self.bls_signers.keys(), pubkey)
            .or_else(|| find_typed::<EcdsaSecretKey>(self.ecdsa_signers.keys(), pubkey))
    }
}

trait GetProxySigner<T: SecretKey> {
    fn get_proxy_signer(&self, pk: &PubKey<T>) -> Option<&ProxySigner<T>>;
}

impl GetProxySigner<BlsSecretKey> for ProxySigners {
    fn get_proxy_signer(&self, pk: &PubKey<BlsSecretKey>) -> Option<&ProxySigner<BlsSecretKey>> {
        self.bls_signers.get(pk)
    }
}

impl GetProxySigner<EcdsaSecretKey> for ProxySigners {
    fn get_proxy_signer(
        &self,
        pk: &PubKey<EcdsaSecretKey>,
    ) -> Option<&ProxySigner<EcdsaSecretKey>> {
        self.ecdsa_signers.get(pk)
    }
}

pub struct SigningManager {
    chain: Chain,
    consensus_signers: HashMap<BlsPublicKey, Signer>,
    proxy_signers: ProxySigners, // HashMap<Vec<u8>, ProxySigner>,
    // proxy_delegations:
    /// Map of module ids to their associated proxy pubkeys.
    /// Used to retrieve the corresponding proxy signer from the signing
    /// manager.
    proxy_pubkeys: HashMap<ModuleId, Vec<GenericPubkey>>,
}

impl SigningManager {
    pub fn new(chain: Chain) -> Self {
        Self {
            chain,
            consensus_signers: Default::default(),
            proxy_signers: Default::default(),
            proxy_pubkeys: Default::default(),
        }
    }

    pub fn add_consensus_signer(&mut self, signer: Signer) {
        self.consensus_signers.insert(signer.pubkey(), signer);
    }

    pub fn add_proxy_signer(&mut self, proxy: GenericProxySigner) {
        self.proxy_signers.add(proxy);
    }

    pub async fn create_proxy<T: SecretKey>(
        &mut self,
        module_id: ModuleId,
        delegator: BlsPublicKey,
    ) -> Result<SignedProxyDelegation, SignerModuleError>
    where
        PubKey<T>: Into<GenericPubkey>,
        ProxySigner<T>: Into<GenericProxySigner>,
    {
        let signer = Signer::<T>::new_random();
        let proxy_pubkey = signer.pubkey().into();

        let message = ProxyDelegation { delegator, proxy: proxy_pubkey };
        let signature = self.sign_consensus(&delegator, &message.tree_hash_root().0).await?;
        let signed_delegation: SignedProxyDelegation = SignedProxyDelegation { signature, message };
        let proxy_signer = ProxySigner::new(signer, signed_delegation).into();

        // Add the new proxy key to the manager's internal state
        self.add_proxy_signer(proxy_signer);
        self.proxy_pubkeys.entry(module_id).or_default().push(proxy_pubkey);

        Ok(signed_delegation)
    }

    // TODO: double check what we can actually sign here with different providers eg
    // web3 signer
    pub async fn sign_consensus(
        &self,
        pubkey: &BlsPublicKey,
        object_root: &[u8; 32],
    ) -> Result<BlsSignature, SignerModuleError> {
        let signer = self
            .consensus_signers
            .get(pubkey)
            .ok_or(SignerModuleError::UnknownConsensusSigner(pubkey.to_vec()))?;
        let signature = signer.sign(self.chain, *object_root).await;

        Ok(signature)
    }

    fn find_proxy(&self, pubkey: &[u8]) -> Option<GenericProxySigner> {
        let generic_pubkey = self.proxy_signers.find_pubkey(pubkey)?;

        let proxy_signer = self.proxy_signers.get(&generic_pubkey).expect("Unreachable!");

        Some(proxy_signer)
    }

    pub async fn sign_proxy(
        &self,
        pubkey: &[u8],
        object_root: &[u8; 32],
    ) -> Result<Vec<u8>, SignerModuleError> {
        let proxy = self
            .find_proxy(pubkey)
            .ok_or(SignerModuleError::UnknownProxySigner(pubkey.to_vec()))?;

        let signature = proxy.sign(self.chain, *object_root).await;

        Ok(signature)
    }

    pub fn consensus_pubkeys(&self) -> Vec<BlsPublicKey> {
        self.consensus_signers.keys().cloned().collect()
    }

    pub fn proxy_pubkeys(&self) -> &HashMap<ModuleId, Vec<GenericPubkey>> {
        &self.proxy_pubkeys
    }

    pub fn has_consensus(&self, pubkey: &BlsPublicKey) -> bool {
        self.consensus_signers.contains_key(pubkey)
    }

    pub fn has_proxy(&self, pubkey: &[u8]) -> bool {
        self.proxy_signers.find_pubkey(pubkey).is_some()
    }

    pub fn get_delegation(
        &self,
        pubkey: &[u8],
    ) -> Result<SignedProxyDelegation, SignerModuleError> {
        let proxy = self
            .find_proxy(pubkey)
            .ok_or(SignerModuleError::UnknownProxySigner(pubkey.to_vec()))?;

        Ok(proxy.delegation())
    }
}

#[cfg(test)]
mod tests {
    use cb_common::signature::compute_signing_root;
    use lazy_static::lazy_static;
    use tree_hash::Hash256;

    use super::*;

    lazy_static! {
        static ref CHAIN: Chain = Chain::Holesky;
        static ref MODULE_ID: ModuleId = ModuleId("SAMPLE_MODULE".to_string());
    }

    fn init_signing_manager() -> (SigningManager, BlsPublicKey) {
        let mut signing_manager = SigningManager::new(*CHAIN);

        let consensus_signer = Signer::new_random();
        let consensus_pk = consensus_signer.pubkey();

        signing_manager.add_consensus_signer(consensus_signer.clone());

        (signing_manager, consensus_pk)
    }

    #[tokio::test]
    async fn test_proxy_key_is_valid_proxy_for_consensus_key() {
        let (mut signing_manager, consensus_pk) = init_signing_manager();

        let signed_delegation = signing_manager
            .create_proxy::<BlsSecretKey>(MODULE_ID.clone(), consensus_pk.clone())
            .await
            .unwrap();

        let validation_result = signed_delegation.validate(*CHAIN);

        assert!(
            validation_result.is_ok(),
            "Proxy delegation signature must be valid for consensus key."
        );

        assert!(
            signing_manager.has_proxy(&signed_delegation.message.proxy.as_ref()),
            "Newly generated proxy key must be present in the signing manager's registry."
        );
    }

    #[tokio::test]
    async fn test_tampered_proxy_key_is_invalid() {
        let (mut signing_manager, consensus_pk) = init_signing_manager();

        let mut signed_delegation = signing_manager
            .create_proxy::<BlsSecretKey>(MODULE_ID.clone(), consensus_pk.clone())
            .await
            .unwrap();

        let m = &mut signed_delegation.signature.0[0];
        (*m, _) = m.overflowing_add(1);

        let validation_result = signed_delegation.validate(*CHAIN);

        assert!(validation_result.is_err(), "Tampered proxy key must be invalid.");
    }

    #[tokio::test]
    async fn test_proxy_key_signs_message() {
        let (mut signing_manager, consensus_pk) = init_signing_manager();

        let signed_delegation = signing_manager
            .create_proxy::<BlsSecretKey>(MODULE_ID.clone(), consensus_pk.clone())
            .await
            .unwrap();
        let proxy_pk = signed_delegation.message.proxy;

        let data_root = Hash256::random();
        let data_root_bytes = data_root.as_fixed_bytes();

        let sig = signing_manager.sign_proxy(proxy_pk.as_ref(), data_root_bytes).await.unwrap();

        // Verify signature

        let domain = CHAIN.builder_domain();
        let signing_root = compute_signing_root(data_root_bytes.tree_hash_root().0, domain);

        let validation_result = proxy_pk.verify_signature(&signing_root, &sig);

        // verify_signed_builder_message(
        //     *CHAIN,
        //     &signed_delegation.message.proxy,
        //     &data_root_bytes,
        //     &sig,
        // );

        assert!(
            validation_result.is_ok(),
            "Proxy keypair must produce valid signatures of messages."
        )
    }
}
