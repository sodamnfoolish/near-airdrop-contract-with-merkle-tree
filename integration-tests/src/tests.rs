use airdrop_merkle_tree_near_rs::MerkleTree;
use borsh::BorshSerialize;
use near_units::parse_near;
use serde_json::json;
use std::{env, fs};
use workspaces::{Account, Contract};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let wasm_arg: &str = &(env::args().nth(1).unwrap());
    let wasm_filepath = fs::canonicalize(env::current_dir()?.join(wasm_arg))?;

    let worker = workspaces::sandbox().await?;
    let wasm = std::fs::read(wasm_filepath)?;

    let airdrop_contract = worker.dev_deploy(&wasm).await?;

    // create accounts and amounts
    let deployer = worker.dev_create_account().await?;

    let mut accounts: Vec<Account> = Vec::new();
    let mut amounts: Vec<u128> = Vec::new();
    let mut items: Vec<Vec<u8>> = Vec::new();

    for i in 0..6 {
        accounts.push(
            deployer
                .create_subaccount(&i.to_string())
                .initial_balance(parse_near!("1 N"))
                .transact()
                .await?
                .into_result()?,
        );

        let amount = parse_near!("1 N");
        amounts.push(amount);

        items.push(
            (accounts.last().unwrap().id().clone(), amount)
                .try_to_vec()
                .unwrap(),
        );
    }

    // create merkle tree
    let merkle_tree = airdrop_merkle_tree_near_rs::MerkleTree::create(&items, None);

    // init airdrop contract
    deployer
        .call(airdrop_contract.id(), "new")
        .args_json(json!({"root_hash" : merkle_tree.root_hash}))
        .transact()
        .await?
        .unwrap();

    // transfer near to airdrop contract
    deployer
        .transfer_near(airdrop_contract.id(), parse_near!("6 N"))
        .await?
        .unwrap();

    // begin tests
    can_claim_ok(
        &deployer,
        &accounts,
        &amounts,
        &airdrop_contract,
        &merkle_tree,
    )
    .await?;
    can_claim_wrong_account_id(
        &deployer,
        &accounts,
        &amounts,
        &airdrop_contract,
        &merkle_tree,
    )
    .await?;
    can_claim_wrong_amount(
        &deployer,
        &accounts,
        &amounts,
        &airdrop_contract,
        &merkle_tree,
    )
    .await?;
    can_claim_wrong_proof(
        &deployer,
        &accounts,
        &amounts,
        &airdrop_contract,
        &merkle_tree,
    )
    .await?;
    claim_ok(&accounts, &amounts, &airdrop_contract, &merkle_tree).await?;
    claim_wrong_amount(&accounts, &amounts, &airdrop_contract, &merkle_tree).await?;
    claim_wrong_proof(&accounts, &amounts, &airdrop_contract, &merkle_tree).await?;

    Ok(())
}

// call can_claim() with correct args
async fn can_claim_ok(
    deployer: &Account,
    accounts: &Vec<Account>,
    amounts: &Vec<u128>,
    airdrop_contract: &Contract,
    merkle_tree: &MerkleTree,
) -> anyhow::Result<()> {
    for i in 0..accounts.len() {
        let response: bool = deployer
            .call(airdrop_contract.id(), "can_claim")
            .args_json(json!({ "account_id": accounts[i].id(), "amount": amounts[i], "proof":  merkle_tree.get_proof(i)}))
            .transact()
            .await?
            .json()?;

        assert!(response);
    }

    println!("can_claim_ok: Passed ✅");

    Ok(())
}

// call can_claim() with wrong account_id
async fn can_claim_wrong_account_id(
    deployer: &Account,
    accounts: &Vec<Account>,
    amounts: &Vec<u128>,
    airdrop_contract: &Contract,
    merkle_tree: &MerkleTree,
) -> anyhow::Result<()> {
    let response: bool = deployer
        .call(airdrop_contract.id(), "can_claim")
        .args_json(json!({ "account_id": accounts[1].id(), "amount": amounts[0], "proof":  merkle_tree.get_proof(0)}))
        .transact()
        .await?
        .json()?;

    assert!(!response);

    println!("can_claim_wrong_account_id: Passed ✅");

    Ok(())
}

// call can_claim() with wrong amount
async fn can_claim_wrong_amount(
    deployer: &Account,
    accounts: &Vec<Account>,
    amounts: &Vec<u128>,
    airdrop_contract: &Contract,
    merkle_tree: &MerkleTree,
) -> anyhow::Result<()> {
    for i in 0..accounts.len() {
        let response: bool = deployer
            .call(airdrop_contract.id(), "can_claim")
            .args_json(json!({ "account_id": accounts[i].id(), "amount": amounts[i] / 2, "proof":  merkle_tree.get_proof(i)}))
            .transact()
            .await?
            .json()?;

        assert!(!response);
    }

    println!("can_claim_wrong_amount: Passed ✅");

    Ok(())
}

// call can_claim() with wrong proof
async fn can_claim_wrong_proof(
    deployer: &Account,
    accounts: &Vec<Account>,
    amounts: &Vec<u128>,
    airdrop_contract: &Contract,
    merkle_tree: &MerkleTree,
) -> anyhow::Result<()> {
    let response: bool = deployer
        .call(airdrop_contract.id(), "can_claim")
        .args_json(json!({ "account_id": accounts[0].id(), "amount": amounts[0], "proof":  merkle_tree.get_proof(1)}))
        .transact()
        .await?
        .json()?;

    assert!(!response);

    println!("can_claim_wrong_proof: Passed ✅");

    Ok(())
}

// call claim with correct args
async fn claim_ok(
    accounts: &Vec<Account>,
    amounts: &Vec<u128>,
    airdrop_contract: &Contract,
    merkle_tree: &MerkleTree,
) -> anyhow::Result<()> {
    for i in 0..accounts.len() {
        let balance_before = accounts[i].view_account().await?.balance;

        let tx = accounts[i]
            .call(airdrop_contract.id(), "claim")
            .args_json(json!({ "amount": amounts[i], "proof":  merkle_tree.get_proof(i)}))
            .transact()
            .await?;

        let mut balance_after_plus_tokens_burnt = accounts[i].view_account().await?.balance;

        for outcome in tx.outcomes() {
            balance_after_plus_tokens_burnt += outcome.tokens_burnt;
        }

        assert_eq!(balance_before + amounts[i], balance_after_plus_tokens_burnt);
    }

    println!("claim_ok: Passed ✅");

    Ok(())
}

// call claim with wrong amount
async fn claim_wrong_amount(
    accounts: &Vec<Account>,
    amounts: &Vec<u128>,
    airdrop_contract: &Contract,
    merkle_tree: &MerkleTree,
) -> anyhow::Result<()> {
    for i in 0..accounts.len() {
        let balance_before = accounts[i].view_account().await?.balance;

        let tx = accounts[i]
            .call(airdrop_contract.id(), "claim")
            .args_json(json!({ "amount": amounts[i] / 2, "proof":  merkle_tree.get_proof(i)}))
            .transact()
            .await?;

        let mut balance_after_plus_tokens_burnt = accounts[i].view_account().await?.balance;

        for outcome in tx.outcomes() {
            balance_after_plus_tokens_burnt += outcome.tokens_burnt;
        }

        assert!(tx.is_failure());
        assert_eq!(balance_before, balance_after_plus_tokens_burnt);
    }

    println!("claim_wrong_amount: Passed ✅");

    Ok(())
}

// call claim with wrong proof
async fn claim_wrong_proof(
    accounts: &Vec<Account>,
    amounts: &Vec<u128>,
    airdrop_contract: &Contract,
    merkle_tree: &MerkleTree,
) -> anyhow::Result<()> {
    let balance_before = accounts[0].view_account().await?.balance;

    let tx = accounts[0]
        .call(airdrop_contract.id(), "claim")
        .args_json(json!({ "amount": amounts[0], "proof":  merkle_tree.get_proof(1)}))
        .transact()
        .await?;

    let mut balance_after_plus_tokens_burnt = accounts[0].view_account().await?.balance;

    for outcome in tx.outcomes() {
        balance_after_plus_tokens_burnt += outcome.tokens_burnt;
    }

    assert!(tx.is_failure());
    assert_eq!(balance_before, balance_after_plus_tokens_burnt);

    println!("claim_wrong_proof: Passed ✅");

    Ok(())
}
