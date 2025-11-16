use std::{
	io::{self, Write},
	time::Duration,
};
use tokio::{sync::oneshot, task::JoinHandle, time::sleep};

/// Minimal spinner inspired by https://github.com/Arteiii/zenity (cli spinners).
pub struct Spinner {
	stop_tx: Option<oneshot::Sender<()>>,
	handle: Option<JoinHandle<()>>,
}

impl Spinner {
	/// Spawn a spinner with the given label. Call [`finish`] to stop it.
	pub fn start(label: impl Into<String>) -> Self {
		let label = label.into();
		let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
		let handle = tokio::spawn(async move {
			let frames = ["⠋", "⠙", "⠚", "⠞", "⠖", "⠦", "⠴", "⠲", "⠳", "⠓"];
			let mut idx = 0usize;
			loop {
				print!("\r{label} {}", frames[idx]);
				let _ = io::stdout().flush();
				idx = (idx + 1) % frames.len();
				tokio::select! {
					_ = sleep(Duration::from_millis(80)) => {},
					_ = &mut stop_rx => {
						print!("\r{label} ✔\n");
						let _ = io::stdout().flush();
						return;
					}
				}
			}
		});
		Self { stop_tx: Some(stop_tx), handle: Some(handle) }
	}

	/// Stop the spinner and optionally print a trailing line.
	pub async fn finish(mut self, message: Option<&str>) {
		if let Some(tx) = self.stop_tx.take() {
			let _ = tx.send(());
		}
		if let Some(handle) = self.handle.take() {
			let _ = handle.await;
		}
		if let Some(msg) = message {
			println!("{msg}");
		}
	}
}

/// Run `future` while a spinner is displayed.
pub async fn with_spinner<F, T>(label: impl Into<String>, fut: F) -> T
where
	F: std::future::Future<Output = T>,
{
	let spinner = Spinner::start(label);
	let out = fut.await;
	spinner.finish(None).await;
	out
}
