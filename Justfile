# Checks for unused dependencies
udeps:
    RUSTC_BOOTSTRAP=1 cargo udeps --all-targets --backend depinfo

# Update dependencies
update-deps:
    cargo update
