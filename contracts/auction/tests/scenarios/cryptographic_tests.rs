//! Real signed authorization and commitment-domain regressions. Only this
//! test account is native; default-mode auctions and SAC transfers are real
//! compiled-WASM/built-in invocations. No mock approval authenticates its calls.
use super::*;
use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    crypto::Hash,
    xdr::{self, WriteXdr},
    Bytes,
};
use std::{
    rc::Rc,
    sync::atomic::{AtomicU32, Ordering},
};

#[contract]
struct Ed25519TestAccount;

#[contractimpl]
impl Ed25519TestAccount {
    pub fn __constructor(env: Env, public_key: BytesN<32>) {
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "public_key"), &public_key);
    }
}

#[contractimpl]
impl CustomAccountInterface for Ed25519TestAccount {
    type Signature = BytesN<64>;
    type Error = soroban_sdk::Error;

    fn __check_auth(
        env: Env,
        signature_payload: Hash<32>,
        signature: Self::Signature,
        _auth_contexts: Vec<Context>,
    ) -> Result<(), Self::Error> {
        let public_key = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "public_key"))
            .unwrap();
        env.crypto().ed25519_verify(
            &public_key,
            &signature_payload.to_bytes().into(),
            &signature,
        );
        Ok(())
    }
}

fn invocation(
    contract: &Address,
    function: &str,
    args: Vec<Val>,
    children: std::vec::Vec<xdr::SorobanAuthorizedInvocation>,
) -> xdr::SorobanAuthorizedInvocation {
    xdr::SorobanAuthorizedInvocation {
        function: xdr::SorobanAuthorizedFunction::ContractFn(xdr::InvokeContractArgs {
            contract_address: contract.into(),
            function_name: function.try_into().unwrap(),
            args: args.into(),
        }),
        sub_invocations: children.try_into().unwrap(),
    }
}

fn signed_entry(
    env: &Env,
    owner: &Address,
    key: &SigningKey,
    root_invocation: xdr::SorobanAuthorizedInvocation,
    nonce: i64,
    expiration: u32,
) -> xdr::SorobanAuthorizationEntry {
    // Soroban authorization signs SHA256(HashIdPreimage XDR), not ScVal XDR.
    // Build the credential explicitly instead of using MockAuth or recording.
    let preimage =
        xdr::HashIdPreimage::SorobanAuthorization(xdr::HashIdPreimageSorobanAuthorization {
            network_id: xdr::Hash(env.ledger().network_id().to_array()),
            nonce,
            signature_expiration_ledger: expiration,
            invocation: root_invocation.clone(),
        });
    let serialized = preimage.to_xdr(xdr::Limits::none()).unwrap();
    let digest = env.crypto().sha256(&Bytes::from_slice(env, &serialized));
    let signature = key.sign(&digest.to_array()).to_bytes();
    xdr::SorobanAuthorizationEntry {
        credentials: xdr::SorobanCredentials::Address(xdr::SorobanAddressCredentials {
            address: owner.into(),
            nonce,
            signature_expiration_ledger: expiration,
            signature: xdr::ScVal::Bytes(signature.to_vec().try_into().unwrap()),
        }),
        root_invocation,
    }
}

fn credentials(entry: &mut xdr::SorobanAuthorizationEntry) -> &mut xdr::SorobanAddressCredentials {
    match &mut entry.credentials {
        xdr::SorobanCredentials::Address(credentials) => credentials,
        _ => unreachable!(),
    }
}

#[test]
fn real_ed25519_credentials_bind_calls_network_expiration_and_replay_nonce() {
    let f = Fixture::new();
    let a = f.auction(&f.config(Mode::Sale, false, 4), 150);
    // Fixed test key material is intentional and never used outside this test.
    let key = SigningKey::from_bytes(&[0x91; 32]);
    let owner = f.e.register(
        Ed25519TestAccount,
        (BytesN::from_array(&f.e, &key.verifying_key().to_bytes()),),
    );
    for asset in [&f.rwa, &f.quote] {
        f.set_auth(&owner, asset, true);
        f.mint(&owner, asset, 1_000);
    }
    f.e.ledger().set_sequence_number(100);
    let client = AuctionClient::new(&f.e, &a);
    let price = 2 * cc::SCALE;
    let quantity = 3i128;
    let deposit = 6i128;
    let transfer = |amount| {
        invocation(
            &f.quote,
            "transfer",
            (owner.clone(), a.clone(), amount).into_val(&f.e),
            std::vec![],
        )
    };
    let place = |children| {
        invocation(
            &a,
            "place_bid",
            (owner.clone(), price, quantity).into_val(&f.e),
            children,
        )
    };
    let root = place(std::vec![transfer(deposit)]);
    let valid = signed_entry(&f.e, &owner, &key, root.clone(), 40_001, 130);
    let before = client.accounting();
    let assert_unchanged = || {
        assert_eq!(client.accounting(), before);
        assert_eq!(client.state().bid_count, 0);
        assert_eq!(client.tick_total(&1), 0);
        assert_eq!(token::Client::new(&f.e, &f.quote).balance(&owner), 1_000);
        assert_eq!(token::Client::new(&f.e, &f.quote).balance(&a), 0);
    };

    let mut forged = valid.clone();
    let mut signature = match &credentials(&mut forged).signature {
        xdr::ScVal::Bytes(bytes) => bytes.to_vec(),
        _ => unreachable!(),
    };
    signature[0] ^= 1;
    credentials(&mut forged).signature = xdr::ScVal::Bytes(signature.try_into().unwrap());
    f.e.set_auths(&[forged]);
    assert!(client.try_place_bid(&owner, &price, &quantity).is_err());
    assert_unchanged();

    // A valid signature for these arguments cannot authorize another price.
    f.e.set_auths(core::slice::from_ref(&valid));
    assert!(client
        .try_place_bid(&owner, &(price + cc::SCALE), &quantity)
        .is_err());
    assert_unchanged();

    // The root signature validates, but its signed SAC child authorizes the
    // wrong deposit. Rejection must roll back both the nonce and auction lock.
    let wrong_child = signed_entry(
        &f.e,
        &owner,
        &key,
        place(std::vec![transfer(deposit + 1)]),
        40_001,
        130,
    );
    f.e.set_auths(&[wrong_child]);
    assert_eq!(
        client.try_place_bid(&owner, &price, &quantity),
        Err(Ok(Error::Transfer))
    );
    assert_unchanged();

    // Mutating either credential field without re-signing changes the digest.
    for change_nonce in [false, true] {
        let mut changed = valid.clone();
        if change_nonce {
            credentials(&mut changed).nonce += 1;
        } else {
            credentials(&mut changed).signature_expiration_ledger += 1;
        }
        f.e.set_auths(&[changed]);
        assert!(client.try_place_bid(&owner, &price, &quantity).is_err());
        assert_unchanged();
    }

    // Even a correctly signed credential is unusable after its expiration.
    let expired = signed_entry(&f.e, &owner, &key, root.clone(), 40_001, 99);
    f.e.set_auths(&[expired]);
    assert!(client.try_place_bid(&owner, &price, &quantity).is_err());
    assert_unchanged();

    let network = f.e.ledger().network_id();
    let mut foreign_network = network.to_array();
    foreign_network[0] ^= 1;
    f.e.ledger().set_network_id(foreign_network);
    f.e.set_auths(core::slice::from_ref(&valid));
    assert!(client.try_place_bid(&owner, &price, &quantity).is_err());
    assert_unchanged();
    f.e.ledger().set_network_id(network.to_array());

    // None of those failures consumes the authentic credential's nonce.
    f.e.set_auths(core::slice::from_ref(&valid));
    assert_eq!(client.place_bid(&owner, &price, &quantity), 0);
    f.clear_auth();
    assert_eq!(token::Client::new(&f.e, &f.quote).balance(&owner), 994);
    assert_eq!(token::Client::new(&f.e, &f.quote).balance(&a), deposit);
    assert_eq!(client.accounting().pending_quote, deposit);

    // Open bids intentionally allow repeated identical orders; only the
    // Soroban credential nonce prevents this signed authorization being reused.
    f.e.set_auths(&[valid]);
    assert!(client.try_place_bid(&owner, &price, &quantity).is_err());
    f.clear_auth();
    assert_eq!(client.state().bid_count, 1);
    assert_eq!(client.accounting().pending_quote, deposit);
    let next = signed_entry(&f.e, &owner, &key, root, 40_002, 130);
    f.e.set_auths(&[next]);
    assert_eq!(client.place_bid(&owner, &price, &quantity), 1);
    f.clear_auth();
    assert_eq!(token::Client::new(&f.e, &f.quote).balance(&owner), 988);

    f.e.ledger().set_sequence_number(110);
    f.close(&a);
    for id in [0, 1] {
        let resolution = client.claim_bid(&id);
        assert_eq!(
            (resolution.rwa, resolution.quote, resolution.refund),
            (2, 4, 2)
        );
    }
    let withdrawal = invocation(
        &a,
        "withdraw",
        (owner.clone(), f.quote.clone(), 4i128).into_val(&f.e),
        std::vec![],
    );
    let entry = signed_entry(&f.e, &owner, &key, withdrawal, 40_003, 130);
    f.e.set_auths(&[entry]);
    assert_eq!(client.withdraw(&owner, &f.quote, &4), 4);
    f.clear_auth();
    assert_eq!(client.credit(&owner, &f.quote), 0);
    assert_eq!(token::Client::new(&f.e, &f.quote).balance(&owner), 992);
    assert_eq!(token::Client::new(&f.e, &f.rwa).balance(&owner), 1_004);
    f.withdraw(&a, &f.issuer, &f.quote, 8);
    assert_eq!(client.accounting().liability_quote, 0);
    assert_eq!(client.accounting().liability_rwa, 0);
    f.check(&a);
}

// An observation outside ledger storage deliberately survives the enclosing
// failed invocation. It proves the authentication callback actually attempted
// reentry, without relying on writes that correctly roll back with that call.
static REENTRY_OBSERVATION: AtomicU32 = AtomicU32::new(0);

#[contract]
struct ReentrantTestAccount;

#[contractimpl]
impl ReentrantTestAccount {
    pub fn __constructor(env: Env, public_key: BytesN<32>, auction: Address) {
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "public_key"), &public_key);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "auction"), &auction);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "attack"), &true);
    }

    // Test instrumentation, not a deployable account or an account policy.
    pub fn disarm(env: Env) {
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "attack"), &false);
    }
}

#[contractimpl]
impl CustomAccountInterface for ReentrantTestAccount {
    type Signature = BytesN<64>;
    type Error = soroban_sdk::Error;

    fn __check_auth(
        env: Env,
        signature_payload: Hash<32>,
        signature: Self::Signature,
        _auth_contexts: Vec<Context>,
    ) -> Result<(), Self::Error> {
        let public_key = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "public_key"))
            .unwrap();
        env.crypto().ed25519_verify(
            &public_key,
            &signature_payload.to_bytes().into(),
            &signature,
        );
        if env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "attack"))
            .unwrap_or(false)
        {
            let auction = env
                .storage()
                .instance()
                .get(&Symbol::new(&env, "auction"))
                .unwrap();
            let nested = AuctionClient::new(&env, &auction).try_place_bid(
                &env.current_contract_address(),
                &(2 * cc::SCALE),
                &1,
            );
            let observation = match nested {
                Err(Ok(Error::Reentry)) => 1, // The auction guard rejected it.
                Err(Err(_)) => 2,             // The host rejected reentry first.
                Err(Ok(_)) => 3,              // Another contract error is insufficient.
                Ok(_) => 4,                   // Reentry unexpectedly succeeded.
            };
            REENTRY_OBSERVATION.store(observation, Ordering::SeqCst);
            // Abandon this authentication attempt after observing the nested
            // rejection. Its nonce and all auction effects must roll back.
            return Err(soroban_sdk::Error::from_contract_error(700));
        }
        Ok(())
    }
}

#[test]
fn signed_authentication_callback_cannot_reenter_and_healthy_retry_preserves_ownership() {
    let f = Fixture::new();
    let a = f.auction(&f.config(Mode::Sale, false, 2), 152);
    let key = SigningKey::from_bytes(&[0x92; 32]);
    let owner = f.e.register(
        ReentrantTestAccount,
        (
            BytesN::from_array(&f.e, &key.verifying_key().to_bytes()),
            a.clone(),
        ),
    );
    for asset in [&f.rwa, &f.quote] {
        f.set_auth(&owner, asset, true);
        f.mint(&owner, asset, 1_000);
    }
    f.e.ledger().set_sequence_number(100);
    let client = AuctionClient::new(&f.e, &a);
    let price = 2 * cc::SCALE;
    let transfer = invocation(
        &f.quote,
        "transfer",
        (owner.clone(), a.clone(), 4i128).into_val(&f.e),
        std::vec![],
    );
    let root = invocation(
        &a,
        "place_bid",
        (owner.clone(), price, 2i128).into_val(&f.e),
        std::vec![transfer],
    );
    let entry = signed_entry(&f.e, &owner, &key, root, 40_010, 130);
    let before = client.accounting();
    REENTRY_OBSERVATION.store(0, Ordering::SeqCst);
    f.e.set_auths(core::slice::from_ref(&entry));
    assert!(client.try_place_bid(&owner, &price, &2).is_err());
    f.clear_auth();
    assert!(matches!(REENTRY_OBSERVATION.load(Ordering::SeqCst), 1 | 2));
    assert_eq!(client.accounting(), before);
    assert_eq!(client.state().bid_count, 0);
    assert_eq!(client.tick_total(&1), 0);
    assert!(client.bid_ids(&owner, &0, &16).is_empty());
    assert_eq!(token::Client::new(&f.e, &f.quote).balance(&owner), 1_000);
    assert_eq!(token::Client::new(&f.e, &f.quote).balance(&a), 0);

    ReentrantTestAccountClient::new(&f.e, &owner).disarm();
    REENTRY_OBSERVATION.store(0, Ordering::SeqCst);
    f.e.set_auths(&[entry]);
    assert_eq!(client.place_bid(&owner, &price, &2), 0);
    f.clear_auth();
    assert_eq!(REENTRY_OBSERVATION.load(Ordering::SeqCst), 0);
    assert_eq!(client.get_bid(&0).owner, owner);
    assert_eq!(client.accounting().pending_quote, 4);
    assert_eq!(token::Client::new(&f.e, &f.quote).balance(&owner), 996);
    assert_eq!(token::Client::new(&f.e, &f.quote).balance(&a), 4);
    f.e.ledger().set_sequence_number(110);
    f.close(&a);
    let delivered = client.claim_bid(&0);
    assert!(delivered.delivered);
    assert_eq!(delivered.rwa, 2);
    assert_eq!(token::Client::new(&f.e, &f.rwa).balance(&owner), 1_002);
    f.withdraw(&a, &f.issuer, &f.quote, 4);
    assert_eq!(client.accounting().liability_quote, 0);
    assert_eq!(client.accounting().liability_rwa, 0);
    f.check(&a);
}

fn fixture_asset(f: &Fixture, sac: &Address) -> xdr::TrustLineAsset {
    // SDK fixture issued SACs use "aaa" with independently generated classic
    // issuers. Find the actual asset by its on-ledger issuer and derived SAC ID,
    // rather than trusting a token's presentation name or a mock trustline.
    for (_, (entry, _)) in f.e.to_ledger_snapshot().ledger_entries {
        let xdr::LedgerEntryData::Account(account) = entry.data else {
            continue;
        };
        let credit = xdr::AlphaNum4 {
            asset_code: xdr::AssetCode4([b'a', b'a', b'a', 0]),
            issuer: account.account_id,
        };
        let asset = xdr::Asset::CreditAlphanum4(credit.clone());
        let encoded = asset.to_xdr(xdr::Limits::none()).unwrap();
        if f.e
            .deployer()
            .with_stellar_asset(Bytes::from_slice(&f.e, &encoded))
            .deployed_address()
            == *sac
        {
            return xdr::TrustLineAsset::CreditAlphanum4(credit);
        }
    }
    panic!("fixture asset issuer missing");
}

fn classic_entry(
    env: &Env,
    owner: &Address,
    key: &SigningKey,
    root: xdr::SorobanAuthorizedInvocation,
    nonce: i64,
) -> xdr::SorobanAuthorizationEntry {
    let mut entry = signed_entry(env, owner, key, root, nonce, 130);
    let credential = credentials(&mut entry);
    // Classic accounts require a vector of {public_key, signature} records,
    // distinct from the custom account's raw BytesN<64> signature argument.
    let signature = xdr::ScVal::Map(Some(
        xdr::ScMap::sorted_from(std::vec![
            xdr::ScMapEntry {
                key: xdr::ScVal::Symbol("public_key".try_into().unwrap()),
                val: xdr::ScVal::Bytes(key.verifying_key().to_bytes().to_vec().try_into().unwrap()),
            },
            xdr::ScMapEntry {
                key: xdr::ScVal::Symbol("signature".try_into().unwrap()),
                val: credential.signature.clone(),
            },
        ])
        .unwrap(),
    ));
    credential.signature = xdr::ScVal::Vec(Some(std::vec![signature].try_into().unwrap()));
    entry
}

#[test]
fn classic_account_master_key_signatures_drive_real_trustline_debits_and_reject_replay() {
    let f = Fixture::new();
    let a = f.auction(&f.config(Mode::Sale, false, 4), 153);
    let key = SigningKey::from_bytes(&[0x93; 32]);
    let account_id = xdr::AccountId(xdr::PublicKey::PublicKeyTypeEd25519(xdr::Uint256(
        key.verifying_key().to_bytes(),
    )));
    let owner: Address = xdr::ScAddress::Account(account_id.clone())
        .try_into_val(&f.e)
        .unwrap();
    let account_entry = xdr::LedgerEntry {
        last_modified_ledger_seq: 90,
        data: xdr::LedgerEntryData::Account(xdr::AccountEntry {
            account_id: account_id.clone(),
            balance: 1_000_000_000,
            seq_num: xdr::SequenceNumber(1),
            num_sub_entries: 2,
            inflation_dest: None,
            flags: 0,
            home_domain: Default::default(),
            thresholds: xdr::Thresholds([1; 4]),
            signers: Default::default(),
            ext: xdr::AccountEntryExt::V0,
        }),
        ext: xdr::LedgerEntryExt::V0,
    };
    f.e.host()
        .add_ledger_entry(
            &Rc::new(xdr::LedgerKey::Account(xdr::LedgerKeyAccount {
                account_id: account_id.clone(),
            })),
            &Rc::new(account_entry),
            None,
        )
        .unwrap();
    for sac in [&f.rwa, &f.quote] {
        let asset = fixture_asset(&f, sac);
        let trustline = xdr::LedgerEntry {
            last_modified_ledger_seq: 90,
            data: xdr::LedgerEntryData::Trustline(xdr::TrustLineEntry {
                account_id: account_id.clone(),
                asset: asset.clone(),
                balance: 1_000,
                limit: 10_000,
                flags: 1, // AUTHORIZED_FLAG; do not substitute contract balances.
                ext: xdr::TrustLineEntryExt::V0,
            }),
            ext: xdr::LedgerEntryExt::V0,
        };
        f.e.host()
            .add_ledger_entry(
                &Rc::new(xdr::LedgerKey::Trustline(xdr::LedgerKeyTrustLine {
                    account_id: account_id.clone(),
                    asset,
                })),
                &Rc::new(trustline),
                None,
            )
            .unwrap();
    }
    assert_eq!(owner.executable(), Some(soroban_sdk::Executable::Account));
    f.e.ledger().set_sequence_number(100);
    let client = AuctionClient::new(&f.e, &a);
    let price = 2 * cc::SCALE;
    let root = invocation(
        &a,
        "place_bid",
        (owner.clone(), price, 3i128).into_val(&f.e),
        std::vec![invocation(
            &f.quote,
            "transfer",
            (owner.clone(), a.clone(), 6i128).into_val(&f.e),
            std::vec![],
        )],
    );
    let valid = classic_entry(&f.e, &owner, &key, root.clone(), 40_100);
    // The signature vector shape is valid but the signature is made with the
    // wrong private key for the listed master public key.
    let wrong_key = SigningKey::from_bytes(&[0x94; 32]);
    let mut forged = classic_entry(&f.e, &owner, &wrong_key, root, 40_100);
    let xdr::ScVal::Vec(Some(signatures)) = &mut credentials(&mut forged).signature else {
        unreachable!();
    };
    let mut signatures = signatures.to_vec();
    let xdr::ScVal::Map(Some(map)) = &mut signatures[0] else {
        unreachable!();
    };
    let mut entries = map.to_vec();
    entries[0].val = xdr::ScVal::Bytes(key.verifying_key().to_bytes().to_vec().try_into().unwrap());
    *map = entries.try_into().unwrap();
    credentials(&mut forged).signature = xdr::ScVal::Vec(Some(signatures.try_into().unwrap()));
    f.e.set_auths(&[forged]);
    assert!(client.try_place_bid(&owner, &price, &3).is_err());
    assert_eq!(client.state().bid_count, 0);
    assert_eq!(token::Client::new(&f.e, &f.quote).balance(&owner), 1_000);
    f.e.set_auths(core::slice::from_ref(&valid));
    assert_eq!(client.place_bid(&owner, &price, &3), 0);
    f.clear_auth();
    assert_eq!(token::Client::new(&f.e, &f.quote).balance(&owner), 994);
    assert_eq!(token::Client::new(&f.e, &f.quote).balance(&a), 6);
    f.e.set_auths(&[valid]);
    assert!(client.try_place_bid(&owner, &price, &3).is_err());
    f.clear_auth();
    assert_eq!(client.state().bid_count, 1);
    assert_eq!(token::Client::new(&f.e, &f.quote).balance(&owner), 994);
    f.e.ledger().set_sequence_number(110);
    f.close(&a);
    let delivered = client.claim_bid(&0);
    assert_eq!(
        (delivered.rwa, delivered.quote, delivered.refund),
        (3, 3, 3)
    );
    let withdrawal = invocation(
        &a,
        "withdraw",
        (owner.clone(), f.quote.clone(), 3i128).into_val(&f.e),
        std::vec![],
    );
    f.e.set_auths(&[classic_entry(&f.e, &owner, &key, withdrawal, 40_101)]);
    assert_eq!(client.withdraw(&owner, &f.quote, &3), 3);
    f.clear_auth();
    assert_eq!(token::Client::new(&f.e, &f.quote).balance(&owner), 997);
    assert_eq!(token::Client::new(&f.e, &f.rwa).balance(&owner), 1_003);
    f.withdraw(&a, &f.issuer, &f.quote, 3);
    f.withdraw(&a, &f.issuer, &f.rwa, 1);
    assert_eq!(client.accounting().liability_quote, 0);
    assert_eq!(client.accounting().liability_rwa, 0);
    f.check(&a);
}

#[derive(Clone)]
struct CommitmentPreimage {
    network: BytesN<32>,
    auction: Address,
    owner: Address,
    nonce: BytesN<32>,
    config_hash: BytesN<32>,
    locked: i128,
    price: i128,
    quantity: i128,
    salt: BytesN<32>,
}

impl CommitmentPreimage {
    fn hash(&self, env: &Env) -> BytesN<32> {
        // Intentionally do not call the production commitment helper: all
        // adversarial field choices must reach the actual reveal verifier.
        let encoded = (
            Symbol::new(env, "CROSSING_BID_V1"),
            self.network.clone(),
            self.auction.clone(),
            self.owner.clone(),
            self.nonce.clone(),
            self.config_hash.clone(),
            self.locked,
            self.price,
            self.quantity,
            self.salt.clone(),
        )
            .to_xdr(env);
        env.crypto().sha256(&encoded).into()
    }
}

#[test]
fn runtime_commitments_bind_network_nonce_locked_and_all_config_terms() {
    let f = Fixture::new();
    let config = f.config(Mode::Sale, true, 15);
    let a = f.auction(&config, 151);
    let client = AuctionClient::new(&f.e, &a);
    let owner = f.user();
    let mut bids = std::vec![];
    f.e.ledger().set_sequence_number(100);
    for index in 0..5 {
        let original = CommitmentPreimage {
            network: f.e.ledger().network_id(),
            auction: a.clone(),
            owner: owner.clone(),
            nonce: BytesN::from_array(&f.e, &[2 * index; 32]),
            config_hash: client.provenance().config_hash,
            locked: 20,
            price: 5 * cc::SCALE,
            quantity: 3,
            salt: BytesN::from_array(&f.e, &[0x70 + index; 32]),
        };
        let mut changed = original.clone();
        match index {
            0 => {
                let mut network = original.network.to_array();
                network[0] ^= 1;
                changed.network = BytesN::from_array(&f.e, &network);
            }
            1 => changed.nonce = BytesN::from_array(&f.e, &[0xe0; 32]),
            2 => changed.locked += 1,
            3 => {
                let mut other_terms = config.clone();
                other_terms.min_fill_quantity += 1;
                changed.config_hash = cc::config_hash(&f.e, &other_terms);
            }
            4 => {
                let mut other_terms = config.clone();
                other_terms.nonreveal_penalty_bps = 2_500;
                changed.config_hash = cc::config_hash(&f.e, &other_terms);
            }
            _ => unreachable!(),
        }
        let hash = changed.hash(&f.e);
        assert_ne!(hash, original.hash(&f.e), "field mutation {index}");
        let bad = f.submit_commit(&a, &owner, &original.nonce, &hash, original.locked);
        // A neighboring valid commitment proves failed reveals do not poison
        // the owner's other records or prevent the auction from completing.
        let mut control = original.clone();
        control.nonce = BytesN::from_array(&f.e, &[2 * index + 1; 32]);
        let good = f.submit_commit(
            &a,
            &owner,
            &control.nonce,
            &control.hash(&f.e),
            control.locked,
        );
        bids.push((bad, good, original, control));
    }
    f.e.ledger().set_sequence_number(110);
    for (index, (bad, good, original, control)) in bids.iter().enumerate() {
        let before = client.accounting();
        let tick = client.tick_total(&4);
        f.signed(
            &owner,
            &a,
            "reveal_bid",
            (
                *bad,
                original.price,
                original.quantity,
                original.salt.clone(),
            )
                .into_val(&f.e),
            &[],
        );
        assert_eq!(
            client.try_reveal_bid(bad, &original.price, &original.quantity, &original.salt),
            Err(Ok(Error::Commitment)),
            "field mutation {index}"
        );
        f.clear_auth();
        assert_eq!(client.accounting(), before);
        assert_eq!(client.tick_total(&4), tick);
        assert!(client.get_bid(bad).quantity.is_none());
        assert_eq!(client.get_bid(bad).nonce, Some(original.nonce.clone()));
        assert_eq!(client.get_bid(bad).deposit, original.locked);

        if index == 0 {
            // A correct commitment also fails on another network, then remains
            // revealable after the original network context is restored.
            let mut other_network = control.network.to_array();
            other_network[0] ^= 1;
            f.e.ledger().set_network_id(other_network);
            f.signed(
                &owner,
                &a,
                "reveal_bid",
                (*good, control.price, control.quantity, control.salt.clone()).into_val(&f.e),
                &[],
            );
            assert_eq!(
                client.try_reveal_bid(good, &control.price, &control.quantity, &control.salt),
                Err(Ok(Error::Commitment))
            );
            f.clear_auth();
            f.e.ledger().set_network_id(control.network.to_array());
            assert_eq!(client.accounting(), before);
            assert_eq!(client.tick_total(&4), tick);
            assert!(client.get_bid(good).quantity.is_none());
        }
        f.reveal(&a, *good, control.price, control.quantity, &control.salt);
        assert_eq!(client.tick_total(&4), tick + control.quantity);
    }
    f.e.ledger().set_sequence_number(115);
    f.close(&a);
    assert_eq!(client.clearing().unwrap().filled, 15);
    for (bad, good, _, _) in bids {
        let refund = client.claim_bid(&bad);
        assert_eq!(refund.reason, cc::ReasonValue::Some(cc::Reason::NonReveal));
        assert_eq!(refund.refund, 20);
        let trade = client.claim_bid(&good);
        assert!(trade.delivered);
        assert_eq!((trade.rwa, trade.quote, trade.refund), (3, 15, 5));
    }
    assert_eq!(client.credit(&owner, &f.quote), 125);
    f.withdraw(&a, &owner, &f.quote, 125);
    f.withdraw(&a, &f.issuer, &f.quote, 75);
    assert_eq!(client.accounting().liability_quote, 0);
    assert_eq!(client.accounting().liability_rwa, 0);
    f.check(&a);
}
