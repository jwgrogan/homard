use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};
use crate::error::{HomardError, Result};

/// macOS Keychain error code: item not found
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

pub fn store_secret(service: &str, account: &str, secret: &str) -> Result<()> {
    // Delete first to handle update case (set_generic_password errors on existing)
    let _ = delete_generic_password(service, account);
    set_generic_password(service, account, secret.as_bytes())
        .map_err(|e| HomardError::Keychain(e.to_string()))?;
    Ok(())
}

pub fn read_secret(service: &str, account: &str) -> Result<Option<String>> {
    match get_generic_password(service, account) {
        Ok(bytes) => {
            let s = String::from_utf8(bytes)
                .map_err(|e| HomardError::Keychain(format!("UTF-8 decode error: {}", e)))?;
            Ok(Some(s))
        }
        Err(e) => {
            // errSecItemNotFound = -25300
            if e.code() == ERR_SEC_ITEM_NOT_FOUND {
                Ok(None)
            } else {
                Err(HomardError::Keychain(e.to_string()))
            }
        }
    }
}

pub fn delete_secret(service: &str, account: &str) -> Result<()> {
    match delete_generic_password(service, account) {
        Ok(()) => Ok(()),
        Err(e) => {
            if e.code() == ERR_SEC_ITEM_NOT_FOUND {
                Ok(()) // already gone
            } else {
                Err(HomardError::Keychain(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keychain_roundtrip() {
        let service = "homard-test-keychain";
        let account = "test-account-roundtrip";
        let _ = delete_secret(service, account);
        store_secret(service, account, "my-secret-value").unwrap();
        let retrieved = read_secret(service, account).unwrap();
        assert_eq!(retrieved, Some("my-secret-value".to_string()));
        delete_secret(service, account).unwrap();
        let after_delete = read_secret(service, account).unwrap();
        assert_eq!(after_delete, None);
    }

    #[test]
    fn test_keychain_missing_returns_none() {
        let result = read_secret("homard-test-keychain", "definitely-does-not-exist-xyz").unwrap();
        assert_eq!(result, None);
    }
}
