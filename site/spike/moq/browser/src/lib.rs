use wasm_bindgen::prelude::*;

/// Dial `url` over the browser's WebTransport, speak MoQ (IETF drafts or moq-lite), and log each frame of
/// `broadcast`/`track` to the console. Returns the negotiated version.
#[wasm_bindgen]
pub async fn watch(url: String, broadcast: String, track: String) -> Result<String, JsError> {
    let url = url::Url::parse(&url)?;
    let alpns: Vec<&str> = moq_net::ALPNS.to_vec();
    let session = web_transport_wasm::ClientBuilder::new()
        .with_protocols(alpns)
        .with_system_roots()
        .connect(url)
        .await
        .map_err(|e| JsError::new(&format!("{e:?}")))?;
    let (origin, origin_driver) = moq_net::origin::Producer::new(Default::default());
    let client = moq_net::Client::new().with_subscriber(origin.clone());
    let now = moq_net::time::Instant::now();
    let (session, driver) = client.connect(now, session).await.map_err(|e| JsError::new(&e.to_string()))?;
    let version = format!("{:?}", session.version());
    web_sys_log(&format!("connected: {version}"));
    wasm_bindgen_futures::spawn_local(async move { let _ = moq_net::time::run(driver).await; });
    wasm_bindgen_futures::spawn_local(async move { let _ = moq_net::time::run(origin_driver).await; });
    let consumer = origin.consume();
    let bc = consumer.routed_broadcast(broadcast.as_str()).await.map_err(|e| JsError::new(&e.to_string()))?;
    let mut sub = bc.track(&track).map_err(|e| JsError::new(&e.to_string()))?.subscribe(None).await.map_err(|e| JsError::new(&e.to_string()))?;
    wasm_bindgen_futures::spawn_local(async move {
        while let Ok(Some(mut group)) = sub.recv_group().await {
            while let Ok(Some(frame)) = group.read_frame().await {
                web_sys_log(&format!("frame {} bytes", frame.payload.len()));
            }
        }
    });
    std::mem::forget(session);
    Ok(version)
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn web_sys_log(s: &str);
}
