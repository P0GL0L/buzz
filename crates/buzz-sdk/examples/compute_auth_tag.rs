//! Compute a NIP-OA auth tag for an agent keypair.
//!
//! Usage:
//!   cargo run --release --example compute_auth_tag -- <owner_secret_hex> <agent_pubkey_hex> [conditions]
//!   printf '%s' "$OWNER_SECRET" |
//!     cargo run --release --example compute_auth_tag -- --owner-secret-stdin <agent_pubkey_hex> [conditions]
//!
//! Prints the JSON auth tag to stdout.

use std::io::Read;

use buzz_sdk::nip_oa;
use nostr::{Keys, PublicKey};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let stdin_secret = args.get(1).is_some_and(|arg| arg == "--owner-secret-stdin");
    if args.len() < 3 {
        eprintln!(
            "Usage: {} <owner_secret_hex> <agent_pubkey_hex> [conditions]\n       \
             {} --owner-secret-stdin <agent_pubkey_hex> [conditions]",
            args[0], args[0]
        );
        std::process::exit(1);
    }

    let (owner_secret, agent_arg_index, conditions_arg_index) = if stdin_secret {
        let mut owner_secret = String::new();
        std::io::stdin()
            .read_to_string(&mut owner_secret)
            .expect("failed to read owner secret from stdin");
        (owner_secret.trim().to_string(), 2, 3)
    } else {
        (args[1].clone(), 2, 3)
    };

    let owner_keys = Keys::parse(&owner_secret).expect("invalid owner secret key");
    let agent_pubkey =
        PublicKey::from_hex(&args[agent_arg_index]).expect("invalid agent pubkey hex");
    let conditions = args
        .get(conditions_arg_index)
        .map(|s| s.as_str())
        .unwrap_or("");

    let tag_json = nip_oa::compute_auth_tag(&owner_keys, &agent_pubkey, conditions)
        .expect("failed to compute auth tag");

    println!("{tag_json}");
}
