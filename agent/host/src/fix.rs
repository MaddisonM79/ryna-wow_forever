//! What fixes a failure, and the words for it a front end shows. The core's error messages name
//! no command: a front end adds its own hint (the CLI appends `run ryna login`; the app shows
//! these words and a Sign in button).

use serde::{Deserialize, Serialize};

/// What the person has to do about a failure, most serious first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Fix {
    /// Sign in again (or, for a developer build, set a device client id).
    SignIn,
    /// Update the agent and the addon: the API no longer takes this format.
    Update,
    /// The API or the sign-in provider couldn't be reached; the next pass tries again.
    Network,
    /// Anything else (a file that doesn't parse, a refusal, the keychain).
    Other,
}

impl Fix {
    /// The neutral words for it, or None for `Other`, whose message is the error itself.
    pub fn advice(self) -> Option<&'static str> {
        match self {
            Fix::SignIn => Some("Sign in again"),
            Fix::Update => Some("Update the agent and addon"),
            Fix::Network => Some("Couldn't reach the API"),
            Fix::Other => None,
        }
    }

    /// What to show for a failure with this fix: the neutral words, or for `Other` the error
    /// itself (its first 200 characters).
    pub fn message(self, detail: &str) -> String {
        match self.advice() {
            Some(words) => words.to_string(),
            None => detail.chars().take(200).collect(),
        }
    }

    /// The CLI's exit code for it: 3 sign in, 4 update, 1 anything else.
    pub fn exit_code(self) -> i32 {
        match self {
            Fix::SignIn => 3,
            Fix::Update => 4,
            Fix::Network | Fix::Other => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_and_order() {
        assert_eq!(Fix::SignIn.message("x"), "Sign in again");
        assert_eq!(Fix::Network.message("timed out"), "Couldn't reach the API");
        assert_eq!(Fix::Other.message("line 3: bad"), "line 3: bad");
        assert_eq!(Fix::Other.message(&"x".repeat(300)).len(), 200);
        assert!(
            Fix::SignIn < Fix::Update && Fix::Update < Fix::Network && Fix::Network < Fix::Other
        );
        assert_eq!(serde_json::to_value(Fix::SignIn).unwrap(), "signIn");
        assert_eq!(
            serde_json::from_value::<Fix>(serde_json::json!("network")).unwrap(),
            Fix::Network
        );
        assert_eq!(Fix::Network.exit_code(), 1);
    }
}
