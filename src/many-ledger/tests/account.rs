pub mod common;
use crate::common::{setup_with_account, setup_with_args, SetupWithAccount, SetupWithArgs};
use many::server::module::account::features::{FeatureInfo, TryCreateFeature};
use many::server::module::account::{self, AccountModuleBackend};
use many::types::identity::testing::identity;
use many::types::VecOrSingle;
use many::Identity;
use many_ledger::module::LedgerModuleImpl;
use std::collections::{BTreeMap, BTreeSet};

fn account_info(
    module_impl: &LedgerModuleImpl,
    id: &Identity,
    account_id: &Identity,
) -> account::InfoReturn {
    let result = AccountModuleBackend::info(
        module_impl,
        id,
        account::InfoArgs {
            account: *account_id,
        },
    );
    assert!(result.is_ok());
    result.unwrap()
}

#[test]
/// Verify we can create an account
fn create() {
    let SetupWithArgs {
        mut module_impl,
        id,
        args,
    } = setup_with_args();
    let result = module_impl.create(&id, args);
    assert!(result.is_ok());
}

#[test]
/// Verify we can't create an account with roles unsupported by feature
fn create_invalid_role() {
    let SetupWithArgs {
        mut module_impl,
        id,
        mut args,
    } = setup_with_args();
    if let Some(roles) = args.roles.as_mut() {
        roles.insert(
            identity(4),
            BTreeSet::from_iter([account::Role::CanLedgerTransact]),
        );
    }
    let result = module_impl.create(&id, args);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code,
        account::errors::unknown_role("").code,
    );
}

#[test]
/// Verify we can change the account description
fn set_description() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account();
    let result = module_impl.set_description(
        &id,
        account::SetDescriptionArgs {
            account: account_id,
            description: "New".to_string(),
        },
    );
    assert!(result.is_ok());
    assert_eq!(
        account_info(&module_impl, &id, &account_id).description,
        Some("New".to_string())
    );
}

#[test]
/// Verify non-owner is not able to change the description
fn set_description_non_owner() {
    let SetupWithAccount {
        mut module_impl,
        account_id,
        ..
    } = setup_with_account();
    let result = module_impl.set_description(
        &identity(1),
        account::SetDescriptionArgs {
            account: account_id,
            description: "Other".to_string(),
        },
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code,
        account::errors::user_needs_role("owner").code
    );
}

#[test]
/// Verify we can list account roles
fn list_roles() {
    let SetupWithAccount {
        module_impl,
        id,
        account_id,
    } = setup_with_account();
    let result = module_impl.list_roles(
        &id,
        account::ListRolesArgs {
            account: account_id,
        },
    );
    assert!(result.is_ok());
    let mut roles = BTreeSet::<account::Role>::new();
    for (_, r) in account_info(&module_impl, &id, &account_id)
        .roles
        .iter_mut()
    {
        roles.append(r)
    }
    roles.remove(&account::Role::Owner);
    assert_eq!(result.unwrap().roles, roles);
}

#[test]
/// Verify we can get given identities account roles
fn get_roles() {
    let SetupWithAccount {
        module_impl,
        id,
        account_id,
    } = setup_with_account();
    let identities = vec![identity(2), identity(3)];
    let result = module_impl.get_roles(
        &id,
        account::GetRolesArgs {
            account: account_id,
            identities: VecOrSingle::from(identities.clone()),
        },
    );
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap().roles,
        account_info(&module_impl, &id, &account_id)
            .roles
            .into_iter()
            .filter(|&(k, _)| identities.contains(&k))
            .collect()
    );
}

#[test]
/// Verify we can add new roles
fn add_roles() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account();
    let new_role = (
        identity(4),
        BTreeSet::from_iter([account::Role::CanLedgerTransact]),
    );
    let result = module_impl.add_roles(
        &id,
        account::AddRolesArgs {
            account: account_id,
            roles: BTreeMap::from_iter([new_role.clone()]),
        },
    );
    assert!(result.is_ok());
    let identities = vec![identity(4)];
    assert!(account_info(&module_impl, &id, &account_id)
        .roles
        .into_iter()
        .find(|&(k, _)| identities.contains(&k))
        .filter(|role| role == &new_role)
        .is_some())
}

#[test]
/// Verify non-owner is not able to add role
fn add_roles_non_owner() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account();
    let mut new_role = BTreeMap::from_iter([(
        identity(4),
        BTreeSet::from_iter([account::Role::CanLedgerTransact]),
    )]);
    let mut roles = account_info(&module_impl, &id, &account_id).roles;
    roles.append(&mut new_role);
    let result = module_impl.add_roles(
        &identity(2),
        account::AddRolesArgs {
            account: account_id,
            roles: new_role.clone(),
        },
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code,
        account::errors::user_needs_role("owner").code
    );
}

#[test]
/// Verify we can remove roles
fn remove_roles() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account();
    let result = module_impl.remove_roles(
        &id,
        account::RemoveRolesArgs {
            account: account_id,
            roles: BTreeMap::from_iter([(
                identity(2),
                BTreeSet::from_iter([account::Role::CanMultisigApprove]),
            )]),
        },
    );
    assert!(result.is_ok());

    let result = module_impl.get_roles(
        &id,
        account::GetRolesArgs {
            account: account_id,
            identities: VecOrSingle::from(vec![identity(2)]),
        },
    );
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap().roles.get(&identity(2)).unwrap(),
        &BTreeSet::<account::Role>::new()
    );
}

#[test]
// Verify non-owner is not able to remove role
fn remove_roles_non_owner() {
    let SetupWithAccount {
        mut module_impl,
        account_id,
        ..
    } = setup_with_account();
    let result = module_impl.remove_roles(
        &identity(2),
        account::RemoveRolesArgs {
            account: account_id,
            roles: BTreeMap::from_iter([(
                identity(2),
                BTreeSet::from_iter([account::Role::CanMultisigApprove]),
            )]),
        },
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code,
        account::errors::user_needs_role("owner").code
    );
}

#[test]
/// Verify we can delete account
fn delete() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account();
    let result = module_impl.delete(
        &id,
        account::DeleteArgs {
            account: account_id,
        },
    );
    assert!(result.is_ok());

    let result = AccountModuleBackend::info(
        &module_impl,
        &id,
        account::InfoArgs {
            account: account_id,
        },
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code,
        account::errors::unknown_account("").code
    );
}

#[test]
/// Verify non-owner is unable to delete account
fn delete_non_owner() {
    let SetupWithAccount {
        mut module_impl,
        account_id,
        ..
    } = setup_with_account();
    let result = module_impl.delete(
        &identity(2),
        account::DeleteArgs {
            account: account_id,
        },
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code,
        account::errors::user_needs_role("owner").code
    );
}

/// Verify that add_feature works with a valid feature.
#[test]
fn add_feature() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account();

    let info_before = account::AccountModuleBackend::info(
        &module_impl,
        &id,
        account::InfoArgs {
            account: account_id,
        },
    )
    .expect("Could not get info");

    // Prevent test from regressing.
    assert!(!info_before
        .features
        .has_id(account::features::ledger::AccountLedger::ID));

    module_impl
        .add_features(
            &id,
            account::AddFeaturesArgs {
                account: account_id,
                roles: None,
                features: account::features::FeatureSet::from_iter([
                    account::features::ledger::AccountLedger.as_feature(),
                ]),
            },
        )
        .expect("Could not add feature");

    let info_after = account::AccountModuleBackend::info(
        &module_impl,
        &id,
        account::InfoArgs {
            account: account_id,
        },
    )
    .expect("Could not get info");

    assert!(info_after
        .features
        .has_id(account::features::ledger::AccountLedger::ID));
}

/// Verify that add_feature works with a valid feature.
#[test]
fn add_feature_non_owner() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account();

    assert!(module_impl
        .add_features(
            &identity(4),
            account::AddFeaturesArgs {
                account: account_id,
                roles: None,
                features: account::features::FeatureSet::from_iter([
                    account::features::ledger::AccountLedger.as_feature(),
                ]),
            },
        )
        .is_err());

    let info_after = account_info(&module_impl, &id, &account_id);

    assert!(!info_after
        .features
        .has_id(account::features::ledger::AccountLedger::ID));
}

/// Verify that add_feature works with a valid feature.
#[test]
fn add_feature_and_role() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account();

    let info_before = account_info(&module_impl, &id, &account_id);
    // Prevent test from regressing.
    assert!(!info_before
        .features
        .has_id(account::features::ledger::AccountLedger::ID));
    assert!(!info_before.roles.contains_key(&identity(4)));

    module_impl
        .add_features(
            &id,
            account::AddFeaturesArgs {
                account: account_id,
                roles: Some(BTreeMap::from_iter([(
                    identity(4),
                    BTreeSet::from_iter([account::Role::Owner]),
                )])),
                features: account::features::FeatureSet::from_iter([
                    account::features::ledger::AccountLedger.as_feature(),
                ]),
            },
        )
        .expect("Could not add feature");

    let info_after = account_info(&module_impl, &id, &account_id);

    assert!(info_after
        .features
        .has_id(account::features::ledger::AccountLedger::ID));
    assert!(info_after
        .roles
        .get(&identity(4))
        .unwrap()
        .contains(&account::Role::Owner));
}

/// Verify that add_feature cannot add existing features.
#[test]
fn add_feature_existing() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account();

    let info_before = account_info(&module_impl, &id, &account_id);

    assert!(info_before
        .features
        .has_id(account::features::multisig::MultisigAccountFeature::ID));

    let result = module_impl.add_features(
        &id,
        account::AddFeaturesArgs {
            account: account_id,
            roles: None,
            features: account::features::FeatureSet::from_iter([
                account::features::multisig::MultisigAccountFeature::default().as_feature(),
            ]),
        },
    );
    assert!(result.is_err());

    let info_after = account_info(&module_impl, &id, &account_id);

    assert!(info_after
        .features
        .has_id(account::features::multisig::MultisigAccountFeature::ID));
}