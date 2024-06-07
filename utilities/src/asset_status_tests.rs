#[cfg(test)]
mod asset_status_tests {
    use super::Asset;
    use super::Status;

    #[test]
    fn test_asset_status_change() {
        let mut asset = Asset::new();
        assert_eq!(asset.get_status(), Status::Pending);

        assert!(asset.activate().is_ok());
        assert_eq!(asset.get_status(), Status::Active);

        assert!(asset.activate().is_err()); // Already active

        assert!(asset.deactivate().is_ok());
        assert_eq!(asset.get_status(), Status::Inactive);

        assert!(asset.deactivate().is_err()); // Already inactive
    }
}
