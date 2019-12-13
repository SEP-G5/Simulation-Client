use crate::hash::{self, Hash, Hashable};
use base64::{decode_config, encode};
use rust_sodium::crypto::sign::{
    self, ed25519::sign, ed25519::verify, ed25519::PublicKey, ed25519::SecretKey,
};
use serde::{Deserialize, Serialize};
use serde_json::{self, json, Value};
use std::fmt::{self, Display, Formatter};
use std::iter::repeat;
use std::time::SystemTime;
// ========================================================================== //

pub type PubKey = Vec<u8>;
pub type Signature = Vec<u8>;

// ========================================================================== //

/// Future work: PubKey and Signature should be fixed size arrays.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Transaction {
    /// bike SN
    id: String,
    /// seconds since unix epoch (1970)
    timestamp: Timestamp,
    pub_key_input: Option<PubKey>,
    pub_key_output: PubKey,
    signature: Signature,
}

// ========================================================================== //

pub type Timestamp = u64;

pub fn make_timestamp() -> Timestamp {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("failed to make timestamp");
    return ts.as_secs() as Timestamp;
}

// ========================================================================== //

/// Helper function to format a Vec<u8> buffer to string as hex rep.
pub fn buf_to_str(buf: &Vec<u8>) -> String {
    let parts: Vec<String> = buf.iter().map(|byte| format!("{:02x}", byte)).collect();
    parts.join("")
}

/// Allow transactions to be printed.
impl Display for Transaction {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let k_input = if self.pub_key_input.is_some() {
            buf_to_str(&self.pub_key_input.as_ref().unwrap())
        } else {
            format!("None")
        };
        write!(
            f,
            "Transaction:{{ id: {}, timestamp: {}, public_key_input: {:#?}, \
             public_key_output: {:#?}, signature: {:#?} }}",
            self.id,
            self.timestamp,
            k_input,
            buf_to_str(&self.pub_key_output),
            buf_to_str(&self.signature)
        )
    }
}

impl Transaction {
    pub fn new(id: String, pub_key_input: Option<PubKey>, pub_key_output: PubKey) -> Transaction {
        Transaction {
            id: id,
            timestamp: make_timestamp(),
            pub_key_input: pub_key_input,
            pub_key_output: pub_key_output,
            signature: Vec::new(),
        }
    }

    pub fn from_details(
        id: String,
        timestamp: Timestamp,
        pub_key_input: Option<PubKey>,
        pub_key_output: PubKey,
        signature: Signature,
    ) -> Transaction {
        Transaction {
            id,
            timestamp,
            pub_key_input,
            pub_key_output,
            signature,
        }
    }

    /// @param id The id of the item, such as serial number of a bike.
    pub fn debug_make_register(id: String) -> (Transaction, SecretKey) {
        let (pk, sk) = sign::gen_keypair();
        let mut t = Transaction::new(id, None, pk.as_ref().to_vec());
        t.sign(&sk);
        (t, sk)
    }

    /// @param t_prev The previous transaction
    /// @param t_sk The previous secret key
    pub fn debug_make_transfer(
        t_prev: &Transaction,
        sk_prev: &SecretKey,
    ) -> (Transaction, SecretKey) {
        let (pk, sk) = sign::gen_keypair();
        let mut t = Transaction {
            id: t_prev.id.clone(),
            timestamp: make_timestamp(),
            pub_key_input: Some(t_prev.pub_key_output.clone()),
            pub_key_output: pk.as_ref().to_vec(),
            signature: Vec::new(),
        };
        t.sign(&sk_prev);
        (t, sk)
    }

    pub fn make_genesis() -> (Transaction, SecretKey) {
        let bytes: Vec<u8> = repeat(0).take(sign::SEEDBYTES).collect();
        let seed = sign::Seed::from_slice(&bytes).expect("Failed to generate seed");
        let (pk, sk) = sign::keypair_from_seed(&seed);
        let mut t = Transaction::new(String::from("GENESIS"), None, pk.as_ref().to_vec());
        t.timestamp = 0;
        t.sign(&sk);
        (t, sk)
    }

    /// Sign a transaction. Make sure all data is filled in, except
    /// signature. Store the signature in itself.
    pub(crate) fn sign(&mut self, sk: &SecretKey) {
        let buf = self.content_to_u8();
        let sig = sign(buf.as_slice(), &sk);
        self.signature = sig;
    }

    /// Verify that this transaction is a valid next transaction, given that the
    /// previous transaction was "prev_t".
    /// @pre "prev_t" must be a valid transaction.
    pub fn verify_is_next(&self, prev_t: &Transaction) -> bool {
        match self.verify() {
            Ok(_) => match &self.pub_key_input {
                Some(key) => &prev_t.pub_key_output == key,
                None => false,
            },
            Err(_) => false,
        }
    }

    /// Verify if the transaction is valid (does the signature match the content?).
    /// There are two types of transactions that are verified differently.
    ///   "Register": There is no input, use the public key of the output.
    ///   "Transfer": There is a input, use the public key of the input.
    pub fn verify(&self) -> Result<(), String> {
        let do_verify = |pk: &[u8], sig: &[u8]| -> Result<(), String> {
            //println!("pk len: {}, sig len: {}", pk.len(), sig.len());
            let pk = PublicKey::from_slice(pk);
            let pk = match pk {
                Some(p) => p,
                None => return Err(format!("could not create public key from input")),
            };
            match verify(sig, &pk) {
                Ok(m) => {
                    let content = self.content_to_u8();
                    if content == m {
                        return Ok(());
                    } else {
                        return Err(format!("content does not match the signature"));
                    }
                }
                Err(_) => return Err(format!("signature is not valid")),
            };
        };

        match &self.pub_key_input {
            Some(pub_key_input) => {
                return do_verify(pub_key_input.as_slice(), self.signature.as_slice());
            }
            None => {
                return do_verify(self.pub_key_output.as_slice(), self.signature.as_slice());
            }
        }
    }

    /// Copy the content of the transaction into a buffer
    fn content_to_u8(&self) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::from(self.id.as_bytes());
        buf.extend_from_slice(&self.timestamp.to_le_bytes());
        if let Some(ref key) = self.pub_key_input {
            buf.extend(key);
        }
        buf.extend(&self.pub_key_output);
        buf
    }

    ///
    pub fn to_json(&self) -> String {
        let mut v: Value = json!({
            "id": self.get_id(),
            "timestamp": self.get_timestamp(),
            "publicKeyInput": Value::Null,
            "publicKeyOutput": encode(self.get_public_key_output()),
            "signature": encode(self.get_signature()),
        });
        if let Some(pk) = self.get_public_key_input() {
            *v.get_mut("publicKeyInput").unwrap() = json!(encode(pk));
        }
        serde_json::to_string_pretty(&v).expect("Failed to convert to json")
    }

    ///
    pub fn from_json(string: &str) -> Result<Self, String> {
        let v = match serde_json::from_str(string) {
            Ok(v) => v,
            Err(_) => return Err(format!("Failed to convert from json")),
        };
        match Transaction::from_json_value(&v) {
            Ok(tx) => Ok(tx),
            Err(_) => Err(format!("Could not convert json to Transaction")),
        }
    }

    ///
    fn from_json_value(v: &Value) -> Result<Self, String> {
        let id: String = match v["id"].as_str() {
            Some(s) => s.to_string(),
            None => return Err(format!("Could not parse id as String")),
        };

        let timestamp: Timestamp = match v["timestamp"].as_u64() {
            Some(v) => v,
            None => return Err(format!("Could not parse id as u64")),
        };

        let pub_key_input: Option<PubKey> = match v["publicKeyInput"].as_str() {
            Some(s) => match decode_config(s, base64::STANDARD) {
                Ok(v) => Some(v),
                Err(e) => {
                    return Err(format!(
                        "Could not decode publicKeyInput from base64 with error: {}",
                        e
                    ))
                }
            },
            None => None,
        };

        let pub_key_output: PubKey = match v["publicKeyOutput"].as_str() {
            Some(s) => match decode_config(s, base64::STANDARD) {
                Ok(v) => v,
                Err(e) => {
                    return Err(format!(
                        "Could not decode publicKeyOutput from base64 with error: {}",
                        e
                    ))
                }
            },
            None => return Err(format!("Could not parse publicKeyOutput as String")),
        };

        let signature: Signature = match v["signature"].as_str() {
            Some(s) => match decode_config(s, base64::STANDARD) {
                Ok(v) => v,
                Err(e) => {
                    return Err(format!(
                        "Could not decode signature from base64 with error: {}",
                        e
                    ))
                }
            },
            None => return Err(format!("Could not parse signature as String")),
        };

        Ok(Transaction::from_details(
            id,
            timestamp,
            pub_key_input,
            pub_key_output,
            signature,
        ))
    }

    /// Returns whether or not the transaction
    pub fn has_input(&self) -> bool {
        self.pub_key_input.is_some()
    }

    /// Returns the ID of the transacted object
    pub fn get_id(&self) -> &String {
        &self.id
    }

    /// Function to set the ID of a transaction. This is only available in test
    /// builds
    #[cfg(test)]
    pub fn set_id(&mut self, id: &str) {
        self.id = String::from(id)
    }

    pub fn get_timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Returns the input public key
    ///
    pub fn get_public_key_input(&self) -> &Option<PubKey> {
        &self.pub_key_input
    }

    /// Returns the output public key
    ///
    pub fn get_public_key_output(&self) -> &PubKey {
        &self.pub_key_output
    }

    pub fn get_signature(&self) -> &Signature {
        &self.signature
    }
}

impl Hashable for Transaction {
    fn calc_hash(&self) -> Hash {
        hash::obj_hash(&self.signature)
    }
}

// Tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_verify() {
        // make a transaction, sign it and verify
        let (mut t, sk) = Transaction::debug_make_register(format!("SN1337BIKE"));
        t.sign(&sk);
        assert_eq!(t.verify(), Ok(()));

        // tamper with the transaction content
        t.id += "1";
        assert_ne!(t.verify(), Ok(()));
    }

    #[test]
    fn test_verify_is_next() {
        // T0 - make the first "register" transaction
        let (t0, sk0) = Transaction::debug_make_register(format!("SN1337BIKE"));
        assert_eq!(t0.verify(), Ok(()));

        // T1 - make the second "transfer" transaction
        let (t1, sk1) = Transaction::debug_make_transfer(&t0, &sk0);
        assert_eq!(t1.verify(), Ok(()));
        assert_eq!(t1.verify_is_next(&t0), true);

        // T2 - make the third "transfer" transaction
        let (t2, _) = Transaction::debug_make_transfer(&t1, &sk1);
        assert_eq!(t2.verify(), Ok(()));
        assert_eq!(t2.verify_is_next(&t1), true);
    }
}
