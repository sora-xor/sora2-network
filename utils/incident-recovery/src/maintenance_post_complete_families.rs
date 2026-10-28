//! PUBLIC FIXTURE ONLY: TON and Parachain ordinary delivery using the already
//! built, mixed-policy runtime-138 artifact. No new runtime policy is compiled.
//! The caller supplies the exact existing Substrate diagnostic manifest inside
//! an explicitly family-scoped wrapper. Every overlay and child is disposable.
use super::*;
use bridge_types::{
    substrate::{JettonAppCall, ParachainAppCall},
    ton::{TonAddress, TonBalance, TonNetworkId, TonTransactionId},
};

const FAMILY_SCHEMA: &str = "sora-nexus-PUBLIC-FIXTURE-post-complete-family-v1";
const ASSET: AssetId = common::DAI;
const CURRENT_DIAGNOSTIC_MANIFEST_SHA: &str =
    "6b635e6c6a59b24ff480cca5bc2d6d815052a5d4e17ec5eba12a3479e3fe13c6";
const CURRENT_RUNTIME_SHA: &str =
    "638fa5b1ebdb9049cd6c0afd80684042c042872573b1c1c7aa345a5a4924416f";

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Family {
    Parachain,
    Ton,
}
impl Family {
    fn network(self) -> GenericNetworkId {
        match self {
            Self::Parachain => GenericNetworkId::Sub(NETWORK),
            Self::Ton => GenericNetworkId::TON(TonNetworkId::Testnet),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Parachain => "parachain",
            Self::Ton => "ton",
        }
    }
    fn sender(self) -> GenericAccount {
        match self {
            Self::Parachain => {
                GenericAccount::Parachain(xcm::VersionedLocation::V4(xcm::v4::Location::here()))
            }
            Self::Ton => GenericAccount::TON(TonAddress::new(0, H256::repeat_byte(6))),
        }
    }
    fn id(self, nonce: u64) -> H256 {
        match self {
            Self::Parachain => MessageId::batched(self.network(), ThisNetworkId::get(), nonce, 0),
            Self::Ton => MessageId::basic(self.network(), ThisNetworkId::get(), nonce),
        }
        .hash()
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FamilyManifest {
    schema: String,
    family: Family,
    input_root: PathBuf,
    diagnostic_manifest: replay::Artifact,
}
fn configs(family: Family) -> Vec<(Vec<u8>, Vec<u8>)> {
    match family {
        Family::Parachain => vec![
            (
                [
                    key(b"ParachainBridgeApp", b"AssetKinds"),
                    NETWORK.encode(),
                    ASSET.encode(),
                ]
                .concat(),
                AssetKind::Sidechain.encode(),
            ),
            (
                [
                    key(b"ParachainBridgeApp", b"SidechainPrecision"),
                    NETWORK.encode(),
                    ASSET.encode(),
                ]
                .concat(),
                18u8.encode(),
            ),
        ],
        Family::Ton => {
            let token = TonAddress::new(0, H256::repeat_byte(29));
            vec![
                (
                    key(b"JettonApp", b"AppInfo"),
                    (
                        TonNetworkId::Testnet,
                        TonAddress::new(0, H256::repeat_byte(31)),
                    )
                        .encode(),
                ),
                (
                    [key(b"JettonApp", b"AssetKinds"), ASSET.encode()].concat(),
                    AssetKind::Sidechain.encode(),
                ),
                (
                    [key(b"JettonApp", b"SidechainPrecision"), ASSET.encode()].concat(),
                    18u8.encode(),
                ),
                (
                    [key(b"JettonApp", b"TokenAddresses"), ASSET.encode()].concat(),
                    token.encode(),
                ),
                (
                    [key(b"JettonApp", b"AssetsByAddresses"), token.encode()].concat(),
                    ASSET.encode(),
                ),
                (
                    bridge_channel::inbound::TONChannelAddresses::<Runtime>::hashed_key_for(
                        TonNetworkId::Testnet,
                    ),
                    TonAddress::new(0, H256::repeat_byte(32)).encode(),
                ),
            ]
        }
    }
}
fn financial(ext: &mut sp_io::TestExternalities, family: Family) -> Result<[u128; 4]> {
    ext.execute_with(|| {
        Ok([
            framenode_runtime::Assets::free_balance(&ASSET, &owner(b"old-owner"))
                .map_err(|e| format!("old balance: {e:?}"))?,
            framenode_runtime::Assets::free_balance(&ASSET, &owner(b"replacement-owner"))
                .map_err(|e| format!("new balance: {e:?}"))?,
            framenode_runtime::Assets::total_issuance(&ASSET)
                .map_err(|e| format!("issuance: {e:?}"))?,
            BridgeProxy::locked_assets(family.network(), ASSET),
        ])
    })
}
fn native_nonce(ext: &mut sp_io::TestExternalities, family: Family) -> u64 {
    ext.execute_with(|| match family {
        Family::Parachain => {
            substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::get(NETWORK)
        }
        Family::Ton => bridge_channel::inbound::ChannelNonces::<Runtime>::get(family.network()),
    })
}
fn outbound(ext: &mut sp_io::TestExternalities, family: Family) -> (u64, bool, Vec<Vec<u8>>) {
    ext.execute_with(|| match family {
        Family::Parachain => (
            substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::get(NETWORK),
            substrate_bridge_channel::outbound::LatestCommitment::<Runtime>::contains_key(NETWORK),
            framenode_runtime::SubstrateBridgeOutboundChannel::recovery_queue(NETWORK)
                .iter()
                .map(Encode::encode)
                .collect(),
        ),
        Family::Ton => (
            bridge_channel::outbound::ChannelNonces::<Runtime>::get(family.network()),
            bridge_channel::outbound::LatestCommitment::<Runtime>::contains_key(family.network()),
            framenode_runtime::BridgeOutboundChannel::recovery_queue(family.network())
                .iter()
                .map(Encode::encode)
                .collect(),
        ),
    })
}
fn protocol_empty(ext: &mut sp_io::TestExternalities, family: Family) -> bool {
    let out = outbound(ext, family);
    ext.execute_with(|| match family {
        Family::Parachain => {
            !substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::contains_key(NETWORK)
                && !substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::contains_key(
                    NETWORK,
                )
        }
        Family::Ton => {
            !bridge_channel::inbound::ChannelNonces::<Runtime>::contains_key(family.network())
                && !bridge_channel::inbound::ReportedChannelNonces::<Runtime>::contains_key(
                    family.network(),
                )
                && !bridge_channel::outbound::ChannelNonces::<Runtime>::contains_key(
                    family.network(),
                )
        }
    }) && out.0 == 0
        && !out.1
        && out.2.is_empty()
}
fn bridge_view(ext: &mut sp_io::TestExternalities, family: Family, id: H256) -> Value {
    let nonce = native_nonce(ext, family);
    let out = outbound(ext, family);
    ext.execute_with(||json!({"nativeInboundNonce":nonce,"nativeOutboundNonce":out.0,"latestCommitmentExists":out.1,
        "acknowledgementQueue":out.2.iter().map(|v|hx(v)).collect::<Vec<_>>(),
        "oldQuery":BridgeProxy::transactions((family.network(),owner(b"old-owner")),id).map(|v|hx(&v.encode())),
        "effectiveQuery":BridgeProxy::transactions((family.network(),owner(b"replacement-owner")),id).map(|v|hx(&v.encode())),
        "senderQuery":BridgeProxy::sender(family.network(),id).map(|v|hx(v.as_ref())),
        "oldSystemAccountExists":frame_system::Account::<Runtime>::contains_key(owner(b"old-owner"))}))
}
fn assert_no_outbound(ext: &mut sp_io::TestExternalities, family: Family) -> Result<()> {
    let out = outbound(ext, family);
    require(
        out.0 == 0 && !out.1 && out.2.is_empty(),
        "family diagnostic created an outbound counter/commitment/message",
    )
}
fn external_amount(family: Family) -> Result<Vec<u8>> {
    match family {
        Family::Parachain => {
            type C = <Runtime as parachain_bridge_app::Config>::BalancePrecisionConverter;
            let (_, amount): (u128, u128) =
                C::to_sidechain(&ASSET, 18, MINT).ok_or("Parachain amount conversion")?;
            require(
                C::from_sidechain(&ASSET, 18, amount).is_some_and(|(v, _)| v == MINT),
                "Parachain exact amount roundtrip",
            )?;
            Ok(amount.encode())
        }
        Family::Ton => {
            type C = <Runtime as jetton_app::Config>::BalancePrecisionConverter;
            let (_, amount): (u128, TonBalance) =
                C::to_sidechain(&ASSET, 18, MINT).ok_or("TON amount conversion")?;
            require(
                C::from_sidechain(&ASSET, 18, amount.clone()).is_some_and(|(v, _)| v == MINT),
                "TON exact amount roundtrip",
            )?;
            Ok(amount.encode())
        }
    }
}
fn make_commitment(
    family: Family,
    recipient: AccountId,
    nonce: u64,
    amount: Vec<u8>,
) -> Result<Commitment> {
    let call = match family {
        Family::Parachain => BridgeCall::ParachainApp(ParachainAppCall::Transfer {
            asset_id: ASSET.into(),
            sender: Some(xcm::VersionedLocation::V4(xcm::v4::Location::here())),
            recipient,
            amount: u128::decode_all(&mut amount.as_slice()).map_err(|e| e.to_string())?,
        }),
        Family::Ton => BridgeCall::JettonApp(JettonAppCall::Transfer {
            token: TonAddress::new(0, H256::repeat_byte(29)).into(),
            sender: TonAddress::new(0, H256::repeat_byte(6)).into(),
            recipient,
            amount: TonBalance::decode_all(&mut amount.as_slice()).map_err(|e| e.to_string())?,
        }),
    };
    let payload = call
        .encode()
        .try_into()
        .map_err(|_| "bounded family payload")?;
    Ok(match family {
        Family::Parachain => GenericCommitment::Sub(bridge_types::substrate::Commitment {
            nonce,
            messages: vec![BridgeMessage {
                payload,
                timepoint: GenericTimepoint::Sora(recovery::CHECKPOINT_NUMBER),
            }]
            .try_into()
            .map_err(|_| "bounded family message")?,
        }),
        Family::Ton => GenericCommitment::TON(bridge_types::ton::Commitment::Inbound(
            bridge_types::ton::InboundCommitment {
                channel: TonAddress::new(0, H256::repeat_byte(32)),
                source: TonAddress::new(0, H256::repeat_byte(31)),
                nonce,
                transaction_id: TonTransactionId {
                    lt: 1,
                    hash: H256::repeat_byte(33),
                },
                payload,
            },
        )),
    })
}
fn proof(family: Family, commitment: &Commitment) -> framenode_runtime::MultiProof {
    match family {
        Family::Parachain => super::proof(commitment),
        Family::Ton => {
            let hash =
                Keccak256::hash_of(&(family.network(), ThisNetworkId::get(), commitment.hash()));
            framenode_runtime::MultiProof::EVMMultisig(multisig_verifier::MultiEVMProof {
                proof: (0u8..4)
                    .map(|n| peer(n).sign_prehashed(hash.as_fixed_bytes()))
                    .collect(),
            })
        }
    }
}
fn native_call(
    family: Family,
    commitment: Commitment,
    proof: framenode_runtime::MultiProof,
) -> RuntimeCall {
    match family {
        Family::Parachain => super::native_call(commitment, proof),
        Family::Ton => RuntimeCall::BridgeInboundChannel(bridge_channel::inbound::Call::submit {
            network_id: family.network(),
            commitment,
            proof,
        }),
    }
}

fn setup(
    source: &Source,
    ext: &mut sp_io::TestExternalities,
    code: &[u8],
    reopen: bool,
    family: Family,
) -> Result<Value> {
    let executor = replay::Executor::builder().build();
    require(
        replay::code(ext)? == source.baseline,
        "source snapshot runtime differs",
    )?;
    assert_complete(ext, &source.policy, true)?;
    let version = replay::version(&executor, ext, &source.baseline)?;
    require(
        (version.spec_version, version.transaction_version) == (138, 134),
        "source version differs",
    )?;
    ext.commit_all().map_err(|e| e.to_string())?;
    let before_financial = financial(ext, family)?;
    let before_liberland = protected_liberland(ext);
    let before_root = replay::root(ext);
    let signer = AccountId::from(relayer().public().0);
    let peers_key = multisig_verifier::PeerKeys::<Runtime>::hashed_key_for(family.network());
    let relayer_key = frame_system::Account::<Runtime>::hashed_key_for(&signer);
    let configs = configs(family);
    require(
        value(ext, &relayer_key).is_none()
            && value(ext, &peers_key).is_none()
            && configs.iter().all(|(k, _)| {
                (matches!(family, Family::Ton) && k == &key(b"JettonApp", b"AppInfo"))
                    || value(ext, k).is_none()
            }),
        "public fixture network or relayer is occupied",
    )?;
    require(
        protocol_empty(ext, family) && before_financial[3] == 0,
        "family diagnostic network has existing protocol state",
    )?;
    let interval = ext.execute_with(framenode_runtime::SubstrateBridgeOutboundChannel::interval);
    let generic_interval = ext.execute_with(framenode_runtime::BridgeOutboundChannel::interval);
    require(
        interval > 0
            && generic_interval > 0
            && !(*source.header.number() + 1).is_multiple_of(interval)
            && !(*source.header.number() + 1).is_multiple_of(generic_interval),
        "diagnostic child must not be an outbound commitment boundary",
    )?;
    let manager =
        BridgeProxy::bridge_account(family.network()).map_err(|e| format!("manager: {e:?}"))?;
    let fee = framenode_runtime::Technical::tech_account_id_to_account_id(
        &BridgeProxy::bridge_fee_tech_account(family.network()),
    )
    .map_err(|e| format!("fee account: {e:?}"))?;
    let custody_financial_before = ext.execute_with(|| {
        [&manager, &fee].map(|account| frame_system::Account::<Runtime>::get(account).data.encode())
    });
    // Exact deployed permissions::Scope::Limited(H512) encoding. The native
    // manage_asset call below must write only these enumerated double-map keys.
    let scope = (0u8, common::hash(&ASSET)).encode();
    let permission_owner = |permission: common::permissions::PermissionId| {
        [
            key(b"Permissions", b"Owners"),
            sp_io::hashing::blake2_256(&permission.encode()).to_vec(),
            sp_io::hashing::blake2_256(&scope).to_vec(),
        ]
        .concat()
    };
    let permission_holder = [
        key(b"Permissions", b"Permissions"),
        sp_io::hashing::blake2_256(&manager.encode()).to_vec(),
        sp_io::hashing::blake2_256(&scope).to_vec(),
    ]
    .concat();
    // Exact native manage_asset write set, plus explicit public fixture config.
    // No broad pallet-prefix exemption and no incoming/outgoing counter edits.
    let mut allowed: BTreeSet<Vec<u8>> = configs.iter().map(|(k, _)| k.clone()).collect();
    allowed.extend([
        peers_key.clone(),
        relayer_key.clone(),
        permission_owner(common::permissions::MINT),
        permission_owner(common::permissions::BURN),
        permission_holder,
        key(b"System", b"Events"),
        key(b"System", b"EventCount"),
    ]);
    for account in [&manager, &fee] {
        allowed.insert(frame_system::Account::<Runtime>::hashed_key_for(account));
        allowed.insert(
            [
                key(b"Technical", b"TechAccounts"),
                sp_io::hashing::blake2_128(&account.encode()).to_vec(),
                account.encode(),
            ]
            .concat(),
        );
    }
    if code != source.baseline {
        allowed.insert(b":code".to_vec());
    }
    if reopen {
        allowed.insert(recovery::BRIDGE_HALT_STORAGE_KEY.to_vec());
    }
    let before: BTreeMap<_, _> = allowed.iter().map(|k| (k.clone(), value(ext, k))).collect();
    ext.execute_with(|| -> Result<()> {
        <BridgeProxy as BridgeAssetRegistry<AccountId, AssetId>>::manage_asset(
            family.network(),
            ASSET,
        )
        .map_err(|e| format!("native diagnostic asset management: {e:?}"))?;
        let peers = (0u8..4).map(|n| peer(n).public()).collect::<BTreeSet<_>>();
        multisig_verifier::PeerKeys::<Runtime>::insert(
            family.network(),
            BoundedBTreeSet::<_, <Runtime as multisig_verifier::Config>::MaxPeers>::try_from(peers)
                .map_err(|_| "public peer bound")?,
        );
        for (k, v) in &configs {
            sp_io::storage::set(k, v);
        }
        sp_io::storage::set(
            &relayer_key,
            &framenode_runtime::AccountData {
                providers: 1,
                ..Default::default()
            }
            .encode(),
        );
        if code != source.baseline {
            sp_io::storage::set(b":code", code);
        }
        if reopen {
            sp_io::storage::clear(recovery::BRIDGE_HALT_STORAGE_KEY);
        }
        Ok(())
    })?;
    let changed: BTreeSet<_> = ext
        .overlayed_changes()
        .changes()
        .map(|(k, _)| k.clone())
        .collect();
    require(
        changed.is_subset(&allowed),
        "unlisted diagnostic setup write",
    )?;
    require(
        financial(ext, family)? == before_financial && protected_liberland(ext) == before_liberland,
        "setup changed financial state or protected Liberland protocol records",
    )?;
    require(
        ext.execute_with(|| {
            [&manager, &fee]
                .map(|account| frame_system::Account::<Runtime>::get(account).data.encode())
                == custody_financial_before
                && frame_system::Account::<Runtime>::get(&signer).data
                    == framenode_runtime::AccountData::default().data
        }),
        "setup changed custody balances or funded the public relayer",
    )?;
    let rows = changed.into_iter().map(|k| {
        let prior=before.get(&k).expect("enumerated key");let after=value(ext,&k);
        json!({"key":hx(&k),"before":if k==b":code" {None} else {prior.as_ref().map(|v|hx(v))},
            "after":if k==b":code" {None} else {after.as_ref().map(|v|hx(v))},
            "beforeSha256":prior.as_ref().map(|v|sha(v)),"afterSha256":after.as_ref().map(|v|sha(v)),
            "beforeBytes":prior.as_ref().map(Vec::len),"afterBytes":after.as_ref().map(Vec::len)})
    }).collect::<Vec<_>>();
    assert_complete(ext, &source.policy, !reopen)?;
    let version = replay::version(&executor, ext, code)?;
    require(
        (version.spec_version, version.transaction_version) == (138, 134),
        "candidate version differs",
    )?;
    Ok(
        json!({"sourceStateRoot":format!("{before_root:#x}"),"overlayStateRoot":format!("{:#x}",replay::root(ext)),
        "writes":rows,"allowedExactKeys":allowed.iter().map(|k|hx(k)).collect::<Vec<_>>(),
        "fixtureNetwork":family.name(),"networkScale":hx(&family.network().encode()),"jettonAppInfoOverriddenOnlyInDisposableOverlay":matches!(family,Family::Ton),"nativeOutboundInterval":interval,"outboundCommitmentBoundary":false,
        "realLiberlandStateUnchanged":true,"protectedLiberland":before_liberland,
        "publicRelayer":hx(signer.as_ref()),"relayerBalanceFunded":false,
        "custodyAccountFinancialDataUnchanged":true,
        "nativeEconomicDispatch":false,"nativeConfigurationOnly":true,
        "bridgeReopenedOnlyInDisposableOverlay":reopen,"counterOrQuarantineWrites":false}),
    )
}

struct Branch {
    name: &'static str,
    candidate: bool,
    reopen: bool,
    mapped: bool,
    success: bool,
}
fn native_failure(event: &RuntimeEvent, family: Family, id: H256) -> bool {
    match (event, family) {
        (
            RuntimeEvent::Dispatch(dispatch::Event::MessageDispatched(message, Err(_))),
            Family::Ton,
        )
        | (
            RuntimeEvent::SubstrateDispatch(dispatch::Event::MessageDispatched(message, Err(_))),
            Family::Parachain,
        ) => message.hash() == id,
        _ => false,
    }
}

fn branch(source: &Source, branch: &Branch, output: &Path, family: Family) -> Result<Value> {
    fs::create_dir(output).map_err(|e| e.to_string())?;
    let code = if branch.candidate {
        &source.candidate
    } else {
        &source.baseline
    };
    let executor = replay::Executor::builder().build();
    let mut ext = source.restore()?;
    let protected = protected_liberland(&mut ext);
    let setup = setup(source, &mut ext, code, branch.reopen, family)?;
    let overlay_root = replay::root(&mut ext);
    let mut parent = source.header.clone();
    parent.set_state_root(overlay_root);
    parent.digest_mut().push(DigestItem::Other(
        [DOMAIN, b"synthetic-parent-not-canonical"].concat(),
    ));
    let mut journal = snapshot::Journal::new(output)?;
    let (initial, overlay_snapshot_sha) = journal.persist(ext)?;
    ext = initial;
    write_new(&output.join("synthetic-parent.scale"), &parent.encode())?;
    write_json(&output.join("setup.json"), &setup)?;
    let header = Header::new(
        *parent.number() + 1,
        H256::zero(),
        H256::zero(),
        parent.hash(),
        Digest {
            logs: vec![DigestItem::PreRuntime(
                *b"BABE",
                (2u8, source.authority_index, source.slot).encode(),
            )],
        },
    );
    let initialized = replay::call(
        &executor,
        &mut ext,
        code,
        "Core_initialize_block",
        &header.encode(),
    )?;
    require(
        ExtrinsicInclusionMode::decode_all(&mut initialized.as_slice())
            .map_err(|e| e.to_string())?
            == ExtrinsicInclusionMode::AllExtrinsics,
        "ordinary final runtime rejected economic admission",
    )?;
    let time = timestamp(source.timestamp)?;
    require(
        apply(&executor, &mut ext, code, &time)? == Ok(Ok(())),
        "ordinary timestamp rejected",
    )?;
    let mut extrinsics = vec![time];
    let before = financial(&mut ext, family)?;
    let nonce = native_nonce(&mut ext, family)
        .checked_add(1)
        .ok_or("inbound nonce overflow")?;
    require(nonce == 1, "unused diagnostic network nonce differs")?;
    let recipient = owner(if branch.mapped {
        b"old-owner"
    } else {
        b"replacement-owner"
    });
    let external_amount = ext.execute_with(|| external_amount(family))?;
    let commitment = make_commitment(family, recipient.clone(), nonce, external_amount.clone())?;
    let original_commitment = commitment.encode();
    let original_proof = proof(family, &commitment);
    let id = family.id(nonce);
    let before_view = bridge_view(&mut ext, family, id);
    let original_call = native_call(family, commitment.clone(), original_proof.clone());
    let public_relayer = relayer();
    let signer = AccountId::from(public_relayer.public().0);
    let mut negatives = Vec::new();
    for (label, case) in [
        ("invalid-outer-signature", SignatureCase::BadSignature),
        ("future-outer-nonce", SignatureCase::WrongNonce),
        ("changed-outer-signed-call", SignatureCase::ChangedPayload),
    ] {
        if !branch.reopen && matches!(case, SignatureCase::WrongNonce) {
            continue;
        }
        let (tx, witness) = signed(
            &mut ext,
            original_call.clone(),
            &public_relayer,
            &parent,
            case,
        )?;
        let prior = replay::root(&mut ext);
        let result = apply(&executor, &mut ext, code, &tx)?;
        let expected = if matches!(case, SignatureCase::WrongNonce) {
            InvalidTransaction::Future
        } else {
            InvalidTransaction::BadProof
        };
        require(
            result == Err(TransactionValidityError::Invalid(expected))
                && replay::root(&mut ext) == prior,
            "invalid outer authorization mutated/admitted state",
        )?;
        negatives.push(json!({"case":label,"outerAuthorization":witness,"result":format!("{result:?}"),"stateRootUnchanged":true}));
    }
    if branch.reopen {
        let mut bad_proof = original_proof.clone();
        if let framenode_runtime::MultiProof::Multisig(p) = &mut bad_proof {
            p.proof[0].0[64] = 255;
        } else if let framenode_runtime::MultiProof::EVMMultisig(p) = &mut bad_proof {
            p.proof[0].0[64] = 255;
        } else {
            return Err("unexpected public proof variant".into());
        }
        let changed = make_commitment(
            family,
            owner(b"external-sender"),
            nonce,
            external_amount.clone(),
        )?;
        let future = make_commitment(
            family,
            recipient.clone(),
            nonce.checked_add(1).ok_or("nonce overflow")?,
            external_amount.clone(),
        )?;
        let cases = [
            (
                "invalid-inner-proof",
                native_call(family, commitment.clone(), bad_proof),
            ),
            (
                "changed-recipient-original-proof",
                native_call(family, changed, original_proof.clone()),
            ),
            (
                "future-native-channel-nonce",
                native_call(family, future.clone(), proof(family, &future)),
            ),
        ];
        for (label, call) in cases {
            let (tx, witness) = signed(
                &mut ext,
                call,
                &public_relayer,
                &parent,
                SignatureCase::Valid,
            )?;
            let prior = replay::root(&mut ext);
            let result = apply(&executor, &mut ext, code, &tx)?;
            require(
                result == Err(TransactionValidityError::Invalid(InvalidTransaction::Call))
                    && replay::root(&mut ext) == prior,
                "invalid native proof/nonce did not fail protocol validation atomically",
            )?;
            negatives.push(json!({"case":label,"outerAuthorization":witness,"result":format!("{result:?}"),
                "stateRootUnchanged":true,"nativeValidationReason":"InvalidTransaction::Call; no fee error accepted"}));
        }
    }
    let (tx, authorization) = signed(
        &mut ext,
        original_call.clone(),
        &public_relayer,
        &parent,
        SignatureCase::Valid,
    )?;
    let before_nonce = ext.execute_with(|| System::account_nonce(&signer));
    let prior = replay::root(&mut ext);
    let result = apply(&executor, &mut ext, code, &tx)?;
    if branch.reopen {
        require(
            result == Ok(Ok(()))
                && ext.execute_with(|| System::account_nonce(&signer)) == before_nonce + 1,
            "authenticated ordinary submit did not pass native fee-exempt admission",
        )?;
        extrinsics.push(tx);
        require(
            native_nonce(&mut ext, family) == nonce,
            "authenticated native delivery did not consume exactly one channel nonce",
        )?;
    } else {
        require(
            result == Err(TransactionValidityError::Invalid(InvalidTransaction::Call))
                && replay::root(&mut ext) == prior,
            "retained bridge halt did not reject atomically through protocol validation",
        )?;
    }
    let after = financial(&mut ext, family)?;
    let after_view = bridge_view(&mut ext, family, id);
    let events = ext.execute_with(System::events);
    if branch.success {
        require(
            after
                == [
                    before[0],
                    before[1].checked_add(MINT).ok_or("balance overflow")?,
                    before[2].checked_add(MINT).ok_or("issuance overflow")?,
                    before[3].checked_add(MINT).ok_or("locked overflow")?,
                ],
            "native transfer did not credit only replacement with exact issuance/locked accounting",
        )?;
        require(
            ext.execute_with(|| {
                BridgeProxy::sender(family.network(), id) == Some(owner(b"replacement-owner"))
                    && BridgeProxy::transactions((family.network(), owner(b"old-owner")), id)
                        .is_none()
                    && BridgeProxy::transactions(
                        (family.network(), owner(b"replacement-owner")),
                        id,
                    )
                    .is_some_and(|r| {
                        r.source() == &family.sender()
                            && r.amount() == MINT
                            && r.asset_id() == &ASSET
                            && r.status() == MessageStatus::Done
                            && r.direction() == MessageDirection::Inbound
                    })
                    && !frame_system::Account::<Runtime>::contains_key(owner(b"old-owner"))
            }),
            "native effective-owner query/source/status or retired account creation differs",
        )?;
    } else {
        require(after == before, "negative branch changed financial state")?;
        if branch.reopen {
            require(
                events
                    .iter()
                    .any(|record| native_failure(&record.event, family, id)),
                "baseline original-recipient native application failure event absent",
            )?;
        }
    }
    require(
        commitment.encode() == original_commitment,
        "signed commitment bytes changed",
    )?;
    assert_no_outbound(&mut ext, family)?;
    let mut replay_witness = None;
    if branch.reopen {
        let (tx, witness) = signed(
            &mut ext,
            original_call,
            &public_relayer,
            &parent,
            SignatureCase::Valid,
        )?;
        let prior = replay::root(&mut ext);
        let result = apply(&executor, &mut ext, code, &tx)?;
        require(
            result == Err(TransactionValidityError::Invalid(InvalidTransaction::Call))
                && replay::root(&mut ext) == prior
                && financial(&mut ext, family)? == after
                && bridge_view(&mut ext, family, id) == after_view,
            "native consumed-identity replay was not rejected without side effects",
        )?;
        replay_witness = Some(
            json!({"authorization":witness,"result":format!("{result:?}"),"stateRootUnchanged":true,"noSecondCredit":true}),
        );
    }
    let encoded = replay::call(
        &executor,
        &mut ext,
        code,
        "BlockBuilder_finalize_block",
        &[],
    )?;
    let finalized = Header::decode_all(&mut encoded.as_slice()).map_err(|e| e.to_string())?;
    require(
        *finalized.number() == *parent.number() + 1
            && *finalized.parent_hash() == parent.hash()
            && *finalized.state_root() == replay::root(&mut ext)
            && *finalized.extrinsics_root()
                == sp_io::trie::blake2_256_ordered_root(
                    extrinsics.iter().map(Encode::encode).collect(),
                    StateVersion::V0,
                ),
        "authored header/extrinsics differ",
    )?;
    assert_complete(&mut ext, &source.policy, !branch.reopen)?;
    assert_no_outbound(&mut ext, family)?;
    require(
        protected_liberland(&mut ext) == protected,
        "diagnostic changed real Liberland queue/counters/tombstones",
    )?;
    let block = RuntimeBlock {
        header: finalized.clone(),
        extrinsics,
    };
    let block_bytes = block.encode();
    write_new(&output.join("block.scale"), &block_bytes)?;
    let mut imported = journal.restore()?;
    let import_executor = replay::Executor::builder().build();
    require(
        replay::call(
            &import_executor,
            &mut imported,
            code,
            "Core_execute_block",
            &block_bytes,
        )?
        .is_empty()
            && replay::root(&mut imported) == *finalized.state_root(),
        "independent overlay import differs",
    )?;
    drop(imported);
    let (persisted, terminal_snapshot_sha) = journal.persist(ext)?;
    drop(persisted);
    drop(journal);
    let mut reopened = snapshot::restore_pinned(
        output,
        "snapshots/000001.json",
        &terminal_snapshot_sha,
        *finalized.state_root(),
    )?;
    require(
        financial(&mut reopened, family)? == after
            && bridge_view(&mut reopened, family, id) == after_view
            && protected_liberland(&mut reopened) == protected,
        "fresh disk state differs",
    )?;
    assert_complete(&mut reopened, &source.policy, !branch.reopen)?;
    assert_no_outbound(&mut reopened, family)?;
    let result = json!({"name":branch.name,"candidateRuntime":branch.candidate,"runtimeSha256":sha(code),
        "sourceStateRoot":format!("{:#x}",source.header.state_root()),"overlayRoot":format!("{overlay_root:#x}"),
        "syntheticParentHeader":hx(&parent.encode()),"syntheticParentHash":format!("{:#x}",parent.hash()),
        "canonicalHistoryReplay":false,"setup":setup,"overlaySnapshotSha256":overlay_snapshot_sha,
        "network":family.name(),"networkScale":hx(&family.network().encode()),"syntheticSenderScale":hx(&family.sender().encode()),"originalRecipient":hx(recipient.as_ref()),
        "effectiveOwner":hx(owner(b"replacement-owner").as_ref()),"localMintBaseUnits":MINT.to_string(),
        "externalAmountScale":hx(&external_amount),"originalCommitmentScale":hx(&original_commitment),
        "originalCommitmentHash":format!("{:#x}",commitment.hash()),"originalProofScale":hx(&original_proof.encode()),
        "messageId":format!("{id:#x}"),"negativeAuthorizations":negatives,"ordinaryAdmission":{"authorization":authorization,
        "result":format!("{result:?}"),"events":format!("{events:?}")},"authenticatedReplay":replay_witness,
        "financialBefore":before.map(|v|v.to_string()),"financialAfter":after.map(|v|v.to_string()),
        "bridgeBefore":before_view,"bridgeAfter":after_view,"expectedReplacementCreditObserved":branch.success,
        "blockSha256":sha(&block_bytes),"blockHash":format!("{:#x}",finalized.hash()),"stateRoot":format!("{:#x}",finalized.state_root()),
        "terminalSnapshotSha256":terminal_snapshot_sha,"independentOverlayImportRootMatches":true,"freshDiskRootMatches":true,
        "realLiberlandStateUnchanged":true,"disposableOutboundCounterUnchanged":true,
        "disposableLatestCommitmentAbsent":true,"productionReady":false});
    write_json(&output.join("result.json"), &result)?;
    Ok(result)
}

pub fn run(manifest: &Path, output: &Path) -> Result<Value> {
    require(
        manifest.is_absolute() && manifest.canonicalize().map_err(|e| e.to_string())? == manifest,
        "family manifest path must be canonical",
    )?;
    let mut bytes = Vec::new();
    fs::File::open(manifest)
        .map_err(|e| e.to_string())?
        .take(MAX_INPUT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    require(bytes.len() <= MAX_INPUT, "family manifest bound")?;
    let input: FamilyManifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    require(
        input.schema == FAMILY_SCHEMA
            && input.input_root.is_absolute()
            && input.input_root.canonicalize().map_err(|e| e.to_string())? == input.input_root,
        "family schema/root",
    )?;
    require(
        input.diagnostic_manifest.sha256 == CURRENT_DIAGNOSTIC_MANIFEST_SHA,
        "only the exact completed v15 diagnostic/artifact binding is admitted",
    )?;
    let original = replay::artifact(&input.input_root, &input.diagnostic_manifest, MAX_INPUT)?;
    let source_path = local_path(&input.input_root, &input.diagnostic_manifest.file)?;
    let source = super::load(&source_path)?;
    require(
        sha(&source.candidate) == CURRENT_RUNTIME_SHA,
        "current reused138 artifact differs",
    )?;
    require(
        fs::read(&source_path).map_err(|e| e.to_string())? == original,
        "inner diagnostic manifest changed",
    )?;
    fs::create_dir(output).map_err(|e| format!("family output must be new: {e}"))?;
    let evidence = json!({"familyManifestSha256":sha(&bytes),"family":input.family.name(),"diagnosticManifest":{"file":input.diagnostic_manifest.file,"sha256":input.diagnostic_manifest.sha256},"source":source.evidence});
    write_json(&output.join("bound-inputs.json"), &evidence)?;
    let mut results = Vec::new();
    for case in [
        Branch {
            name: "baseline-halted",
            candidate: false,
            reopen: false,
            mapped: true,
            success: false,
        },
        Branch {
            name: "baseline-original-recipient",
            candidate: false,
            reopen: true,
            mapped: true,
            success: false,
        },
        Branch {
            name: "baseline-live-recipient-control",
            candidate: false,
            reopen: true,
            mapped: false,
            success: true,
        },
        Branch {
            name: "candidate-halted",
            candidate: true,
            reopen: false,
            mapped: true,
            success: false,
        },
        Branch {
            name: "candidate-original-recipient",
            candidate: true,
            reopen: true,
            mapped: true,
            success: true,
        },
    ] {
        results.push(branch(
            &source,
            &case,
            &output.join(case.name),
            input.family,
        )?);
    }
    require(
        fs::read(manifest).map_err(|e| e.to_string())? == bytes
            && replay::artifact(&input.input_root, &input.diagnostic_manifest, MAX_INPUT)?
                == original,
        "family inputs changed",
    )?;
    require(
        replay::artifact(
            &source.manifest.input_root,
            &source.manifest.candidate_runtime,
            MAX_INPUT,
        )? == source.candidate
            && replay::artifact(
                &source.manifest.input_root,
                &source.manifest.candidate_policy,
                recovery::MAX_INHERENT_BYTES * 3,
            )? == source.policy.encode(),
        "reused runtime/policy changed during diagnostic",
    )?;
    let report = json!({"schema":FAMILY_SCHEMA,"family":input.family.name(),"source":evidence,"branches":results,
        "nativeExecution":false,"nativeConfigurationSetupOnly":true,"exactWasmOrdinaryChannelExecution":true,"fixtureOnly":true,"productionReady":false,
        "sourceHistoryModified":false,"canonicalHistoryReplay":false,"productionBridgeReopeningAuthorized":false,"realCounterpartReopeningVerified":false,
        "consensusSealsVerified":false,"operatorApprovalVerified":false,"runtimeArtifactRebuilt":false,
        "limits":["Exact existing mixed-policy138 Wasm, used in a disposable overlay restored from the immutable mixed39 fixture.",
        "Synthetic Rococo Parachain or TON Testnet application configuration and deterministic public peers; no real source chain or external finality is authenticated.",
        "TON AppInfo is explicitly replaced only in the disposable overlay; all setup preimages are recorded. No incoming/outgoing nonce or quarantine marker is edited.",
        "No new runtime policy, production halt change, counterpart reconciliation, or old-owner signing proof is established.",
        "These ordinary-delivery probes do not establish maintenance catch-up or attacker exclusion for this family."]});
    write_json(&output.join("report.json"), &report)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn family_diagnostic_failure_binds_structured_message_id_and_dispatch_instance() {
        let ton_id = MessageId::basic(Family::Ton.network(), ThisNetworkId::get(), 1);
        let sub_id = MessageId::batched(Family::Parachain.network(), ThisNetworkId::get(), 1, 0);
        let error = sp_runtime::DispatchError::Other("Retired recovery account");
        let ton = RuntimeEvent::Dispatch(dispatch::Event::MessageDispatched(ton_id, Err(error)));
        let sub =
            RuntimeEvent::SubstrateDispatch(dispatch::Event::MessageDispatched(sub_id, Err(error)));
        assert!(native_failure(&ton, Family::Ton, ton_id.hash()));
        assert!(native_failure(&sub, Family::Parachain, sub_id.hash()));
        assert!(!native_failure(&ton, Family::Parachain, ton_id.hash()));
        assert!(!native_failure(&sub, Family::Ton, sub_id.hash()));
        assert!(!native_failure(&ton, Family::Ton, Family::Ton.id(2)));
        assert!(!native_failure(
            &RuntimeEvent::Dispatch(dispatch::Event::MessageDispatched(ton_id, Ok(()))),
            Family::Ton,
            ton_id.hash(),
        ));
        assert!(!native_failure(
            &RuntimeEvent::SubstrateDispatch(dispatch::Event::MessageRejected(sub_id)),
            Family::Parachain,
            sub_id.hash(),
        ));
    }

    #[test]
    fn family_diagnostic_original_payload_proof_binds_recipient_and_native_domain() {
        for family in [Family::Parachain, Family::Ton] {
            let amount = match family {
                Family::Parachain => 1u128.encode(),
                Family::Ton => TonBalance::new(1).encode(),
            };
            let original = make_commitment(family, owner(b"old-owner"), 1, amount.clone()).unwrap();
            let changed = make_commitment(family, owner(b"replacement-owner"), 1, amount).unwrap();
            let proof = proof(family, &original);
            let digest = |c: &Commitment| match family {
                Family::Parachain => Keccak256::hash_of(&AuxiliaryDigest {
                    logs: vec![AuxiliaryDigestItem::Commitment(
                        ThisNetworkId::get(),
                        c.hash(),
                    )],
                }),
                Family::Ton => {
                    Keccak256::hash_of(&(family.network(), ThisNetworkId::get(), c.hash()))
                }
            };
            let signatures = if let framenode_runtime::MultiProof::Multisig(p) = proof {
                p.proof
            } else if let framenode_runtime::MultiProof::EVMMultisig(p) = proof {
                p.proof
            } else {
                panic!("unexpected public proof")
            };
            assert_eq!(signatures.len(), 4);
            for (index, signature) in signatures.iter().enumerate() {
                assert!(ecdsa::Pair::verify_prehashed(
                    signature,
                    digest(&original).as_fixed_bytes(),
                    &peer(index as u8).public()
                ));
                assert!(!ecdsa::Pair::verify_prehashed(
                    signature,
                    digest(&changed).as_fixed_bytes(),
                    &peer(index as u8).public()
                ));
            }
            assert_ne!(original.encode(), changed.encode());
            assert_ne!(family.id(1), family.id(2));
        }
    }
    #[test]
    fn family_diagnostic_config_keys_are_exact_unique_and_never_nonce_or_quarantine() {
        for family in [Family::Parachain, Family::Ton] {
            let rows = configs(family);
            let keys = rows
                .iter()
                .map(|(key, _)| key.clone())
                .collect::<BTreeSet<_>>();
            assert_eq!(keys.len(), rows.len());
            assert_eq!(
                rows.len(),
                if matches!(family, Family::Ton) { 6 } else { 2 }
            );
            for (k, v) in rows {
                assert!(!v.is_empty());
                assert!(!k.starts_with(b":sora-nexus:"));
                for pallet in [
                    b"BridgeInboundChannel".as_slice(),
                    b"BridgeOutboundChannel",
                    b"SubstrateBridgeInboundChannel",
                    b"SubstrateBridgeOutboundChannel",
                ] {
                    assert!(!k.starts_with(&key(pallet, b"ChannelNonces")));
                    assert!(!k.starts_with(&key(pallet, b"ReportedChannelNonces")));
                }
            }
        }
    }
    #[test]
    fn family_diagnostic_requires_explicit_family_and_rejects_unknown_manifest_fields() {
        let base = json!({"schema":FAMILY_SCHEMA,"family":"ton","inputRoot":"/public-fixture","diagnosticManifest":{"file":"source.json","sha256":"0".repeat(64)}});
        assert!(serde_json::from_value::<FamilyManifest>(base.clone()).is_ok());
        let mut missing = base.clone();
        missing.as_object_mut().unwrap().remove("family");
        assert!(serde_json::from_value::<FamilyManifest>(missing).is_err());
        let mut extra = base;
        extra["allowPolicyMismatch"] = json!(true);
        assert!(serde_json::from_value::<FamilyManifest>(extra).is_err());
    }
}
