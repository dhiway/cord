use origin_rs::OriginClient;

fn node_url() -> Option<String> {
	std::env::var("ORIGIN_NODE_URL").ok()
}

#[tokio::test]
async fn dynamic_connect_and_metadata() {
	let Some(url) = node_url() else {
		eprintln!("skipping (set ORIGIN_NODE_URL to run)");
		return;
	};

	let client = OriginClient::connect(&url).await.expect("connect");
	let meta = client.metadata();
	assert!(meta.version() >= 16, "metadata version should be >=16 for views");
}

#[tokio::test]
async fn dynamic_storage_system_number() {
	let Some(url) = node_url() else {
		eprintln!("skipping (set ORIGIN_NODE_URL to run)");
		return;
	};

	let client = OriginClient::connect(&url).await.expect("connect");
	let block_no = client.storage_value("System", "Number", vec![]).await.expect("fetch storage");
	assert!(block_no.is_some(), "System.Number should exist");
}

#[tokio::test]
async fn dynamic_storage_system_number_typed() {
	let Some(url) = node_url() else {
		eprintln!("skipping (set ORIGIN_NODE_URL to run)");
		return;
	};

	let client = OriginClient::connect(&url).await.expect("connect");
	let block_no: Option<u32> =
		client.storage_value_as("System", "Number", vec![], &["u32"]).await.ok();
	assert!(block_no.is_some(), "typed System.Number should decode");
}
