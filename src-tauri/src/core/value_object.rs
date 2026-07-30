use crate::core::result::Result;

/// Trait for value objects.
/// Value objects are immutable, equality by value, and must validate themselves.
pub trait ValueObject: Clone + PartialEq + Eq + Send + Sync + 'static {
    /// Validate the value object's invariants.
    fn validate(&self) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::result::AppError;

    #[derive(Clone, PartialEq, Eq, Debug)]
    struct Email(String);

    impl ValueObject for Email {
        fn validate(&self) -> Result<()> {
            if self.0.contains('@') && self.0.contains('.') {
                Ok(())
            } else {
                Err(AppError::Validation("Invalid email".to_string()))
            }
        }
    }

    #[test]
    fn valid_email() {
        let email = Email("user@example.com".to_string());
        assert!(email.validate().is_ok());
    }

    #[test]
    fn invalid_email() {
        let email = Email("not-an-email".to_string());
        assert!(email.validate().is_err());
    }

    #[test]
    fn value_object_equality() {
        let e1 = Email("a@b.com".to_string());
        let e2 = Email("a@b.com".to_string());
        let e3 = Email("c@d.com".to_string());

        assert_eq!(e1, e2);
        assert_ne!(e1, e3);
    }

    #[test]
    fn value_object_clone() {
        let e1 = Email("a@b.com".to_string());
        let e2 = e1.clone();
        assert_eq!(e1, e2);
    }
}
