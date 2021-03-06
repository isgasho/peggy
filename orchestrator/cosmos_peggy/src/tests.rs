use crate::query::*;
use crate::send::*;
use actix::Arbiter;
use actix::System;
use clarity::PrivateKey as EthPrivateKey;
use contact::client::Contact;
use deep_space::coin::Coin;
use deep_space::private_key::PrivateKey;
use peggy_proto::peggy::query_client::QueryClient as PeggyQueryClient;
use rand::Rng;
use std::time::Duration;
use tonic::transport::Channel;

/// If you run the start-chains.sh script in the peggy repo it will pass
/// port 1317 on localhost through to the peggycli rest-server which can
/// then be used to run this test and debug things quickly. You will need
/// to run the following command and copy a phrase so that you actually
/// have some coins to send funds
/// docker exec -it peggy_test_instance cat /validator-phrases
#[test]
#[ignore]
fn test_endpoints() {
    env_logger::init();
    let key = PrivateKey::from_phrase("ski choice subject cage color ritual critic jeans vintage praise school nature lend inject laptop cost chimney auction cliff surprise outside dumb demand hollow", "").unwrap();
    let token_name = "footoken".to_string();
    let mut rng = rand::thread_rng();
    let secret: [u8; 32] = rng.gen();
    let eth_private_key = EthPrivateKey::from_slice(&secret).expect("Failed to parse eth key");
    let fee = Coin {
        denom: token_name,
        amount: 500_000u32.into(),
    };

    let res = System::run(move || {
        let contact = Contact::new("http://localhost:1317", Duration::from_secs(30));
        Arbiter::spawn(async move {
            let mut grpc_client = PeggyQueryClient::connect("localhost:26657").await.unwrap();
            let res = test_valset_request_calls(
                &contact,
                &mut grpc_client,
                key,
                eth_private_key,
                fee.clone(),
            )
            .await;
            if res.is_err() {
                println!("{:?}", res);
                System::current().stop_with_code(1);
            }

            System::current().stop();
        });
    });

    if let Err(e) = res {
        panic!(format!("{:?}", e))
    }
}

async fn test_valset_request_calls(
    contact: &Contact,
    grpc_client: &mut PeggyQueryClient<Channel>,
    key: PrivateKey,
    eth_private_key: EthPrivateKey,
    fee: Coin,
) -> Result<(), String> {
    // next we update our eth address so that we can be sure it's present in the resulting valset
    // request
    let res = update_peggy_eth_address(&contact, eth_private_key, key, fee.clone()).await;
    if res.is_err() {
        return Err(format!("Failed to update eth address {:?}", res));
    }

    let res = get_valset(grpc_client, 1u32.into()).await;
    if res.is_ok() {
        return Err(format!(
            "Got valset request that should not exist {:?}",
            res
        ));
    }

    // we request a valset be created
    // and then look at results at two block heights, one where the request was made, one where it
    // was not
    let res = send_valset_request(&contact, key, fee.clone()).await;
    if res.is_err() {
        return Err(format!("Failed to create valset request {:?}", res));
    }
    let valset_request_block = res.unwrap().height;

    let res = get_valset(grpc_client, valset_request_block).await;
    println!("valset response is {:?}", res);
    if let Ok(Some(valset)) = res {
        let addresses = valset.filter_empty_addresses().0;
        if !addresses.contains(&eth_private_key.to_public_key().unwrap()) {
            // we successfully submitted our eth address before, we should find it now
            return Err(format!(
                "Incorrect Valset, {:?} does not include submitted eth address",
                valset
            ));
        }
    } else {
        return Err(format!(
            "Failed to get valset {} that should exist",
            valset_request_block
        ));
    }
    let res = get_valset(grpc_client, valset_request_block).await;
    println!("valset response is {:?}", res);
    if let Ok(Some(valset)) = res {
        let addresses = valset.filter_empty_addresses().0;
        if !addresses.contains(&eth_private_key.to_public_key().unwrap()) {
            // we successfully submitted our eth address before, we should find it now
            return Err("Incorrect Valset, does not include submitted eth address".to_string());
        }

        // issue here, we can't actually test valset confirm because all the validators need
        // to have submitted an Ethereum address first.
        let res = send_valset_confirm(
            &contact,
            eth_private_key,
            fee,
            valset,
            key,
            "test".to_string(),
        )
        .await;
        if res.is_err() {
            return Err(format!("Failed to send valset confirm {:?}", res));
        }
    } else {
        return Err("Failed to get valset request that should exist".to_string());
    }

    // valset confirm

    Ok(())
}

/// simple test used to get raw signature bytes to feed into other
/// applications for testing. Specifically to get signing compatibility
/// with go-ethereum
#[test]
#[ignore]
fn get_sig() {
    use rand::Rng;
    use sha3::{Digest, Keccak256};
    let mut rng = rand::thread_rng();
    let secret: [u8; 32] = rng.gen();
    let eth_private_key = EthPrivateKey::from_slice(&secret).expect("Failed to parse eth key");
    let eth_address = eth_private_key.to_public_key().unwrap();
    let msg = eth_address.as_bytes();
    let eth_signature = eth_private_key.sign_ethereum_msg(msg);
    let digest = Keccak256::digest(msg);
    trace!(
        "sig: 0x{} hash: 0x{} address: 0x{}",
        clarity::utils::bytes_to_hex_str(&eth_signature.to_bytes()),
        clarity::utils::bytes_to_hex_str(&digest),
        clarity::utils::bytes_to_hex_str(eth_address.as_bytes())
    );
}
