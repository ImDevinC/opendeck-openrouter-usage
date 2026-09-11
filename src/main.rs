use openaction::*;

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

const CREDITS_URL: &str = "https://openrouter.ai/api/v1/credits";

/// How long a cached balance stays fresh before the ticker refetches it.
const REFETCH_INTERVAL: Duration = Duration::from_secs(300);
/// How often the ticker wakes up to look for stale instances.
const TICK_INTERVAL: Duration = Duration::from_secs(60);

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct CreditSettings {
	/// OpenRouter management API key. The plugin adds the `Bearer ` prefix.
	api_key: String,
}

/// Per-instance state tracked by the plugin so the background ticker can
/// refresh every visible key without waiting on the next inbound event.
#[derive(Clone, Default)]
struct InstanceState {
	settings: CreditSettings,
	remaining: Option<f64>,
	last_fetch: Option<Instant>,
}

static INSTANCES: LazyLock<Mutex<HashMap<String, InstanceState>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Deserialize)]
struct ApiResponse {
	data: CreditsData,
}

#[derive(Deserialize)]
struct CreditsData {
	total_credits: f64,
	total_usage: f64,
}

/// Remaining balance is the credits purchased minus the credits used.
fn remaining_credits(total_credits: f64, total_usage: f64) -> f64 {
	total_credits - total_usage
}

fn format_currency(value: f64) -> String {
	format!("${:.2}", value)
}

fn render_svg(amount: &str) -> String {
	format!(
		r##"<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144" viewBox="0 0 144 144">
  <text x="72" y="62" text-anchor="middle" fill="#9a9a9a" font-family="sans-serif" font-size="12" font-weight="600">CREDITS</text>
  <text x="72" y="98" text-anchor="middle" fill="#2ecc71" font-family="sans-serif" font-size="26" font-weight="bold">{amount}</text>
</svg>"##,
		amount = amount,
	)
}

fn render_svg_error(message: &str) -> String {
	let short: String = message.chars().take(16).collect();
	format!(
		r##"<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144" viewBox="0 0 144 144">
  <text x="72" y="76" text-anchor="middle" fill="#e74c3c" font-family="sans-serif" font-size="18" font-weight="bold">err</text>
  <text x="72" y="98" text-anchor="middle" fill="#9a9a9a" font-family="sans-serif" font-size="12">{short}</text>
</svg>"##,
		short = short,
	)
}

fn render_svg_no_key() -> String {
	r##"<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144" viewBox="0 0 144 144">
  <text x="72" y="76" text-anchor="middle" fill="#f1c40f" font-family="sans-serif" font-size="16" font-weight="bold">SET KEY</text>
</svg>"##
		.to_string()
}

fn to_data_uri(svg: &str) -> String {
	let b64 = base64::engine::general_purpose::STANDARD.encode(svg.as_bytes());
	format!("data:image/svg+xml;base64,{}", b64)
}

async fn send_image(instance: &Instance, svg: &str, label: &str) {
	let uri = to_data_uri(svg);
	log::debug!(
		"[{}] sending {label} image (uri len {})\nsvg: {}\nuri prefix: {}",
		instance.instance_id,
		uri.len(),
		svg,
		&uri[..uri.len().min(40)],
	);
	let result = instance.set_image(Some(uri), None).await;
	if let Err(e) = result {
		log::error!("[{}] set_image failed: {}", instance.instance_id, e);
	}
}

async fn fetch_remaining(api_key: &str) -> Result<f64, String> {
	log::debug!("fetch_remaining: requesting {}", CREDITS_URL);
	let resp = CLIENT
		.get(CREDITS_URL)
		.header("Authorization", format!("Bearer {}", api_key))
		.send()
		.await
		.map_err(|e| {
			log::warn!("fetch_remaining: request failed: {}", e);
			e.to_string()
		})?;

	if !resp.status().is_success() {
		log::warn!("fetch_remaining: non-success status {}", resp.status());
		return Err(format!("HTTP {}", resp.status()));
	}

	let body = resp.text().await.map_err(|e| {
		log::warn!("fetch_remaining: could not decode response body: {:#}", e);
		format!("{:#}", e)
	})?;

	let parsed: ApiResponse = serde_json::from_str(&body).map_err(|e| {
		let preview: String = body.chars().take(200).collect();
		log::warn!("fetch_remaining: json parse failed: {}\nbody preview: {}", e, preview);
		format!("{}", e)
	})?;

	let remaining = remaining_credits(parsed.data.total_credits, parsed.data.total_usage);
	log::debug!(
		"fetch_remaining: total_credits={} total_usage={} remaining={}",
		parsed.data.total_credits,
		parsed.data.total_usage,
		remaining,
	);
	Ok(remaining)
}

/// Render the current state. If no balance is cached, fetch synchronously
/// and fall back to an error message if that fails.
async fn set_credit_image(instance: &Instance, state: &InstanceState) {
	log::debug!(
		"[{}] set_credit_image: key_set={} cached={:?}",
		instance.instance_id,
		!state.settings.api_key.trim().is_empty(),
		state.remaining,
	);

	if state.settings.api_key.trim().is_empty() {
		send_image(instance, &render_svg_no_key(), "no-key").await;
		return;
	}

	match state.remaining {
		Some(v) => send_image(instance, &render_svg(&format_currency(v)), "credits").await,
		None => match fetch_remaining(&state.settings.api_key).await {
			Ok(v) => {
				// Store the fresh result so the background ticker doesn't refetch.
				if let Some(s) = INSTANCES.lock().unwrap().get_mut(&instance.instance_id) {
					s.remaining = Some(v);
					s.last_fetch = Some(Instant::now());
				}
				send_image(instance, &render_svg(&format_currency(v)), "credits").await;
			}
			Err(e) => {
				log::warn!("[{}] set_credit_image: fetch failed: {}", instance.instance_id, e);
				send_image(instance, &render_svg_error(&e), "error").await;
			}
		},
	}
}

struct CreditsAction;

#[async_trait]
impl Action for CreditsAction {
	const UUID: ActionUuid = "com.imdevinc.openroutercredits.credits";
	type Settings = CreditSettings;

	async fn will_appear(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		log::debug!(
			"[{}] will_appear: key_set={}",
			instance.instance_id,
			!settings.api_key.trim().is_empty(),
		);
		let id = instance.instance_id.clone();
		let state = {
			let mut map = INSTANCES.lock().unwrap();
			let entry = map.entry(id.clone()).or_default();
			entry.settings = settings.clone();
			entry.clone()
		};

		set_credit_image(instance, &state).await;
		Ok(())
	}

	async fn will_disappear(&self, instance: &Instance, _settings: &Self::Settings) -> OpenActionResult<()> {
		log::debug!("[{}] will_disappear", instance.instance_id);
		INSTANCES.lock().unwrap().remove(&instance.instance_id);
		Ok(())
	}

	async fn did_receive_settings(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		log::debug!(
			"[{}] did_receive_settings: key_set={}",
			instance.instance_id,
			!settings.api_key.trim().is_empty(),
		);
		let id = instance.instance_id.clone();
		let state = {
			let mut map = INSTANCES.lock().unwrap();
			let entry = map.entry(id.clone()).or_default();
			// Settings changed; drop the cached balance so the next render refetches.
			entry.settings = settings.clone();
			entry.remaining = None;
			entry.last_fetch = None;
			entry.clone()
		};

		set_credit_image(instance, &state).await;
		Ok(())
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		// Manual refresh on press.
		log::debug!("[{}] key_up: manual refresh", instance.instance_id);
		let state = InstanceState {
			settings: settings.clone(),
			remaining: None,
			last_fetch: None,
		};
		set_credit_image(instance, &state).await;
		Ok(())
	}
}

async fn tick_instances() {
	let now = Instant::now();

	// Fetch the visible instances without holding our lock across an await.
	let visible = visible_instances(CreditsAction::UUID).await;

	// Snapshot each instance + its tracked state so we don't hold the lock
	// across async work.
	let snapshot: Vec<(Arc<Instance>, InstanceState)> = {
		let map = INSTANCES.lock().unwrap();
		visible
			.into_iter()
			.filter_map(|instance| {
				let id = instance.instance_id.clone();
				map.get(&id).map(|state| (instance, state.clone()))
			})
			.collect()
	};
	log::debug!("tick_instances: {} visible instances", snapshot.len());

	for (instance, mut state) in snapshot {
		if state.settings.api_key.trim().is_empty() {
			continue;
		}

		let stale = state
			.last_fetch
			.map(|t| now.duration_since(t) >= REFETCH_INTERVAL)
			.unwrap_or(true);
		if !stale {
			continue;
		}

		match fetch_remaining(&state.settings.api_key).await {
			Ok(v) => {
				state.remaining = Some(v);
				state.last_fetch = Some(now);
				if let Some(s) = INSTANCES.lock().unwrap().get_mut(&instance.instance_id) {
					*s = state.clone();
				}
				send_image(&instance, &render_svg(&format_currency(v)), "credits").await;
			}
			Err(e) => {
				log::warn!("[{}] tick: fetch failed: {}", instance.instance_id, e);
				send_image(&instance, &render_svg_error(&e), "error").await;
			}
		}
	}
}

#[tokio::main]
async fn main() -> OpenActionResult<()> {
	{
		use simplelog::*;
		if let Err(error) = TermLogger::init(
			LevelFilter::Debug,
			Config::default(),
			TerminalMode::Stdout,
			ColorChoice::Never,
		) {
			eprintln!("Logger initialization failed: {}", error);
		}
	}

	register_action(CreditsAction).await;

	// Background refresher: refetch stale balances at most once every 5 minutes.
	let ticker = tokio::spawn(async {
		log::debug!("background ticker started (60s interval)");
		let mut interval = tokio::time::interval(TICK_INTERVAL);
		loop {
			interval.tick().await;
			tick_instances().await;
		}
	});

	let result = run(std::env::args().collect()).await;
	ticker.abort();
	result
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn remaining_is_credits_minus_usage() {
		assert_eq!(remaining_credits(100.5, 25.75), 74.75);
		assert_eq!(remaining_credits(10.0, 10.0), 0.0);
	}

	#[test]
	fn currency_formats_two_decimals() {
		assert_eq!(format_currency(74.75), "$74.75");
		assert_eq!(format_currency(100.5), "$100.50");
		assert_eq!(format_currency(0.0), "$0.00");
	}

	#[test]
	fn parses_sample_api_payload() {
		// Mirrors the real GET /credits response.
		let body = r#"{
			"data": {
				"total_credits": 100.5,
				"total_usage": 25.75
			}
		}"#;
		let parsed: ApiResponse = serde_json::from_str(body).expect("sample payload must deserialize");
		assert_eq!(remaining_credits(parsed.data.total_credits, parsed.data.total_usage), 74.75);
	}

	#[test]
	fn render_shows_amount() {
		let svg = render_svg("$74.75");
		assert!(svg.contains("$74.75"));
		assert!(svg.contains("CREDITS"));
	}

	#[test]
	fn data_uri_is_svg() {
		let uri = to_data_uri(&render_svg_no_key());
		assert!(uri.starts_with("data:image/svg+xml;base64,"));
	}

	#[test]
	fn error_truncation_is_char_safe() {
		let svg = render_svg_error("this is a very long message that should be truncated!");
		assert!(svg.contains("this is a very l"));
	}
}
