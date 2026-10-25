//! Classic issuer and multisignature authentication against real SAC storage.
//! Account entries are fixture state; every authorization under test is a real
//! signed Soroban credential verified by the host, without mock authentication.
use super::*;
use ed25519_dalek::{Signer as _, SigningKey};
use soroban_sdk::{
    xdr::{self, WriteXdr},
    Bytes,
};
use std::rc::Rc;

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

fn signed(
    env: &Env,
    owner: &Address,
    keys: &[&SigningKey],
    root: xdr::SorobanAuthorizedInvocation,
    nonce: i64,
    expiration: u32,
) -> xdr::SorobanAuthorizationEntry {
    let preimage =
        xdr::HashIdPreimage::SorobanAuthorization(xdr::HashIdPreimageSorobanAuthorization {
            network_id: xdr::Hash(env.ledger().network_id().to_array()),
            nonce,
            signature_expiration_ledger: expiration,
            invocation: root.clone(),
        });
    let digest = env.crypto().sha256(&Bytes::from_slice(
        env,
        &preimage.to_xdr(xdr::Limits::none()).unwrap(),
    ));
    // Preserve caller ordering so malformed order and duplicate records really
    // reach the host's classic-account verifier rather than being normalized.
    let signatures: std::vec::Vec<_> = keys
        .iter()
        .map(|key| {
            xdr::ScVal::Map(Some(
                xdr::ScMap::sorted_from(std::vec![
                    xdr::ScMapEntry {
                        key: xdr::ScVal::Symbol("public_key".try_into().unwrap()),
                        val: xdr::ScVal::Bytes(
                            key.verifying_key().to_bytes().to_vec().try_into().unwrap()
                        ),
                    },
                    xdr::ScMapEntry {
                        key: xdr::ScVal::Symbol("signature".try_into().unwrap()),
                        val: xdr::ScVal::Bytes(
                            key.sign(&digest.to_array())
                                .to_bytes()
                                .to_vec()
                                .try_into()
                                .unwrap()
                        ),
                    },
                ])
                .unwrap(),
            ))
        })
        .collect();
    xdr::SorobanAuthorizationEntry {
        credentials: xdr::SorobanCredentials::Address(xdr::SorobanAddressCredentials {
            address: owner.into(),
            nonce,
            signature_expiration_ledger: expiration,
            signature: xdr::ScVal::Vec(Some(signatures.try_into().unwrap())),
        }),
        root_invocation: root,
    }
}

fn fixture_asset(f: &Fixture, sac: &Address) -> xdr::TrustLineAsset {
    for (_, (entry, _)) in f.e.to_ledger_snapshot().ledger_entries {
        let xdr::LedgerEntryData::Account(account) = entry.data else {
            continue;
        };
        let credit = xdr::AlphaNum4 {
            asset_code: xdr::AssetCode4([b'a', b'a', b'a', 0]),
            issuer: account.account_id,
        };
        let encoded = xdr::Asset::CreditAlphanum4(credit.clone())
            .to_xdr(xdr::Limits::none())
            .unwrap();
        if f.e
            .deployer()
            .with_stellar_asset(Bytes::from_slice(&f.e, &encoded))
            .deployed_address()
            == *sac
        {
            return xdr::TrustLineAsset::CreditAlphanum4(credit);
        }
    }
    panic!("issued fixture asset missing");
}

fn classic_account(f: &Fixture, master: &SigningKey, signers: &[&SigningKey]) -> Address {
    let account_id = xdr::AccountId(xdr::PublicKey::PublicKeyTypeEd25519(xdr::Uint256(
        master.verifying_key().to_bytes(),
    )));
    let mut extra: std::vec::Vec<_> = signers
        .iter()
        .map(|key| xdr::Signer {
            key: xdr::SignerKey::Ed25519(xdr::Uint256(key.verifying_key().to_bytes())),
            weight: 1,
        })
        .collect();
    extra.sort();
    let threshold = if signers.is_empty() {
        [1; 4]
    } else {
        [0, 2, 2, 2]
    };
    let account = xdr::LedgerEntry {
        last_modified_ledger_seq: 90,
        data: xdr::LedgerEntryData::Account(xdr::AccountEntry {
            account_id: account_id.clone(),
            balance: 1_000_000_000,
            seq_num: xdr::SequenceNumber(1),
            num_sub_entries: 2 + signers.len() as u32,
            inflation_dest: None,
            flags: 0,
            home_domain: Default::default(),
            thresholds: xdr::Thresholds(threshold),
            signers: extra.try_into().unwrap(),
            ext: xdr::AccountEntryExt::V0,
        }),
        ext: xdr::LedgerEntryExt::V0,
    };
    f.e.host()
        .add_ledger_entry(
            &Rc::new(xdr::LedgerKey::Account(xdr::LedgerKeyAccount {
                account_id: account_id.clone(),
            })),
            &Rc::new(account),
            None,
        )
        .unwrap();
    for sac in [&f.rwa, &f.quote] {
        let asset = fixture_asset(f, sac);
        let line = xdr::LedgerEntry {
            last_modified_ledger_seq: 90,
            data: xdr::LedgerEntryData::Trustline(xdr::TrustLineEntry {
                account_id: account_id.clone(),
                asset: asset.clone(),
                balance: 1_000,
                limit: 10_000,
                flags: 1,
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
                &Rc::new(line),
                None,
            )
            .unwrap();
    }
    xdr::ScAddress::Account(account_id)
        .try_into_val(&f.e)
        .unwrap()
}

#[test]
fn classic_issuer_signatures_bind_factory_configuration_salt_and_funding_transfer() {
    for mode in [Mode::Sale, Mode::Buyback] {
        let f = Fixture::new();
        let key = SigningKey::from_bytes(&[0xa1; 32]);
        let issuer = classic_account(&f, &key, &[]);
        // Setup transfers SAC administrative control; calls under test below
        // authenticate the actual classic current administrator cryptographically.
        f.signed(
            &f.issuer,
            &f.rwa,
            "set_admin",
            (issuer.clone(),).into_val(&f.e),
            &[],
        );
        token::StellarAssetClient::new(&f.e, &f.rwa).set_admin(&issuer);
        f.clear_auth();
        let mut config = f.config(mode, false, 7);
        config.issuer = issuer.clone();
        config.buyback_recipient = issuer.clone();
        let salt = BytesN::from_array(&f.e, &[0xa2; 32]);
        let factory = crossing_auction_factory::AuctionFactoryClient::new(&f.e, &f.factory);
        let predicted = factory.predict(&issuer, &salt);
        let root = invocation(
            &f.factory,
            "create",
            (config.clone(), salt.clone()).into_val(&f.e),
            std::vec![],
        );
        let create = signed(&f.e, &issuer, &[&key], root, 70_001, 130);
        let mut changed = config.clone();
        changed.supply_or_budget += 1;
        f.e.set_auths(core::slice::from_ref(&create));
        assert!(factory.try_create(&changed, &salt).is_err());
        let other_salt = BytesN::from_array(&f.e, &[0xa3; 32]);
        f.e.set_auths(core::slice::from_ref(&create));
        assert!(factory.try_create(&config, &other_salt).is_err());
        assert_eq!(factory.count(), 0);
        assert!(factory.get_auction(&predicted).is_none());
        f.e.set_auths(core::slice::from_ref(&create));
        assert_eq!(factory.create(&config, &salt), predicted);
        f.e.set_auths(&[create]);
        assert!(factory.try_create(&config, &salt).is_err());
        f.clear_auth();
        assert_eq!(factory.count(), 1);
        let client = AuctionClient::new(&f.e, &predicted);
        assert_eq!(client.config(), config);
        assert_eq!(
            factory.get_auction(&predicted).unwrap().config_hash,
            cc::config_hash(&f.e, &config)
        );

        let authorize = invocation(
            &f.rwa,
            "set_authorized",
            (predicted.clone(), true).into_val(&f.e),
            std::vec![],
        );
        f.e.set_auths(&[signed(&f.e, &issuer, &[&key], authorize, 70_002, 130)]);
        token::StellarAssetClient::new(&f.e, &f.rwa).set_authorized(&predicted, &true);
        f.clear_auth();
        f.set_auth(&predicted, &f.quote, true);
        let source = if mode == Mode::Sale { &f.rwa } else { &f.quote };
        let funding = |amount| {
            invocation(
                &predicted,
                "fund",
                ().into_val(&f.e),
                std::vec![invocation(
                    source,
                    "transfer",
                    (issuer.clone(), predicted.clone(), amount).into_val(&f.e),
                    std::vec![],
                )],
            )
        };
        let before = client.accounting();
        f.e.set_auths(&[signed(&f.e, &issuer, &[&key], funding(8i128), 70_003, 130)]);
        assert_eq!(client.try_fund(), Err(Ok(Error::Transfer)));
        assert_eq!(client.state().stage, Stage::Created);
        assert_eq!(client.accounting(), before);
        assert_eq!(token::Client::new(&f.e, source).balance(&issuer), 1_000);
        assert_eq!(token::Client::new(&f.e, source).balance(&predicted), 0);

        // Current SAC administrative ownership is rechecked after creation.
        let transfer_admin = invocation(
            &f.rwa,
            "set_admin",
            (f.issuer.clone(),).into_val(&f.e),
            std::vec![],
        );
        f.e.set_auths(&[signed(&f.e, &issuer, &[&key], transfer_admin, 70_004, 130)]);
        token::StellarAssetClient::new(&f.e, &f.rwa).set_admin(&f.issuer);
        let valid = signed(&f.e, &issuer, &[&key], funding(7i128), 70_003, 130);
        f.e.set_auths(core::slice::from_ref(&valid));
        assert_eq!(client.try_fund(), Err(Ok(Error::Unauthorized)));
        assert_eq!(client.accounting(), before);
        f.signed(
            &f.issuer,
            &f.rwa,
            "set_admin",
            (issuer.clone(),).into_val(&f.e),
            &[],
        );
        token::StellarAssetClient::new(&f.e, &f.rwa).set_admin(&issuer);
        f.e.set_auths(core::slice::from_ref(&valid));
        client.fund();
        f.clear_auth();
        assert_eq!(client.state().stage, Stage::Funded);
        assert_eq!(token::Client::new(&f.e, source).balance(&issuer), 993);
        assert_eq!(token::Client::new(&f.e, source).balance(&predicted), 7);
        f.e.set_auths(&[valid]);
        assert!(client.try_fund().is_err());
        f.clear_auth();
        assert_eq!(token::Client::new(&f.e, source).balance(&predicted), 7);
        f.check(&predicted);
    }
}

#[test]
fn classic_multisig_enforces_weights_order_membership_expiry_and_atomic_nonce_rollback() {
    let f = Fixture::new();
    let a = f.auction(&f.config(Mode::Sale, false, 4), 174);
    let master = SigningKey::from_bytes(&[0xb0; 32]);
    let first = SigningKey::from_bytes(&[0xb1; 32]);
    let second = SigningKey::from_bytes(&[0xb2; 32]);
    let outsider = SigningKey::from_bytes(&[0xb3; 32]);
    let owner = classic_account(&f, &master, &[&first, &second]);
    let mut keys = [&first, &second];
    keys.sort_by_key(|key| key.verifying_key().to_bytes());
    f.e.ledger().set_sequence_number(100);
    let client = AuctionClient::new(&f.e, &a);
    let price = 2 * cc::SCALE;
    let root = |amount| {
        invocation(
            &a,
            "place_bid",
            (owner.clone(), price, 3i128).into_val(&f.e),
            std::vec![invocation(
                &f.quote,
                "transfer",
                (owner.clone(), a.clone(), amount).into_val(&f.e),
                std::vec![],
            )],
        )
    };
    let before = client.accounting();
    let assert_unchanged = || {
        assert_eq!(client.accounting(), before);
        assert_eq!(client.state().bid_count, 0);
        assert_eq!(client.tick_total(&1), 0);
        assert_eq!(token::Client::new(&f.e, &f.quote).balance(&owner), 1_000);
        assert_eq!(token::Client::new(&f.e, &f.quote).balance(&a), 0);
    };
    let mut with_outsider = std::vec![keys[0], keys[1], &outsider];
    with_outsider.sort_by_key(|key| key.verifying_key().to_bytes());
    for signatures in [
        std::vec![],                 // Empty credentials cannot authenticate an account.
        std::vec![keys[0]],          // Only half the required weight.
        std::vec![&master],          // The master key has zero configured weight.
        std::vec![keys[0], keys[0]], // Duplicates cannot inflate weight.
        std::vec![keys[1], keys[0]], // Records must be strictly ordered.
        with_outsider,               // An outsider invalidates even sufficient valid weight.
    ] {
        f.e.set_auths(&[signed(&f.e, &owner, &signatures, root(6i128), 71_001, 130)]);
        assert!(client.try_place_bid(&owner, &price, &3).is_err());
        assert_unchanged();
    }
    f.e.set_auths(&[signed(&f.e, &owner, &keys, root(6i128), 71_001, 99)]);
    assert!(client.try_place_bid(&owner, &price, &3).is_err());
    assert_unchanged();
    f.e.set_auths(&[signed(&f.e, &owner, &keys, root(7i128), 71_001, 130)]);
    assert_eq!(
        client.try_place_bid(&owner, &price, &3),
        Err(Ok(Error::Transfer))
    );
    assert_unchanged();
    let valid = signed(&f.e, &owner, &keys, root(6i128), 71_001, 130);
    f.e.set_auths(core::slice::from_ref(&valid));
    assert_eq!(client.place_bid(&owner, &price, &3), 0);
    f.clear_auth();
    assert_eq!(token::Client::new(&f.e, &f.quote).balance(&owner), 994);
    f.e.set_auths(&[valid]);
    assert!(client.try_place_bid(&owner, &price, &3).is_err());
    f.clear_auth();
    assert_eq!(client.state().bid_count, 1);
    f.e.ledger().set_sequence_number(110);
    f.close(&a);
    let delivery = client.claim_bid(&0);
    assert_eq!((delivery.rwa, delivery.quote, delivery.refund), (3, 3, 3));
    let withdrawal = invocation(
        &a,
        "withdraw",
        (owner.clone(), f.quote.clone(), 3i128).into_val(&f.e),
        std::vec![],
    );
    f.e.set_auths(&[signed(&f.e, &owner, &keys, withdrawal, 71_002, 130)]);
    assert_eq!(client.withdraw(&owner, &f.quote, &3), 3);
    f.clear_auth();
    assert_eq!(token::Client::new(&f.e, &f.quote).balance(&owner), 997);
    assert_eq!(token::Client::new(&f.e, &f.rwa).balance(&owner), 1_003);
    f.withdraw(&a, &f.issuer, &f.quote, 3);
    f.withdraw(&a, &f.issuer, &f.rwa, 1);
    assert_eq!(client.accounting().liability_rwa, 0);
    assert_eq!(client.accounting().liability_quote, 0);
    f.check(&a);
}
