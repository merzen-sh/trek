use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::network::CookieParam;
use chromiumoxide::page::Page;
use futures_util::StreamExt;
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep};

const SSO_URL: &str = "https://portal-api.cfx.re/v1/auth/discourse?return=";
const CREATED_ASSETS_URL: &str = "https://portal.cfx.re/assets/created-assets";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

pub(crate) struct BrowserSession {
    browser: Browser,
    handler: JoinHandle<()>,
}

impl BrowserSession {
    pub(crate) async fn launch() -> Result<Self> {
        let config = BrowserConfig::builder()
            .no_sandbox()
            .build()
            .map_err(|error| anyhow!("failed to find or configure Chromium: {error}"))?;
        let (browser, handler) = Browser::launch(config)
            .await
            .context("failed to launch browser")?;

        let handler = tokio::spawn(async move {
            let mut handler = handler;
            while let Some(event) = handler.next().await {
                if event.is_err() {
                    break;
                }
            }
        });

        Ok(Self { browser, handler })
    }

    pub(crate) async fn authenticate(&self, forum_cookie: &str) -> Result<()> {
        let page = self
            .browser
            .new_page("about:blank")
            .await
            .context("failed to open browser page")?;

        page.goto(SSO_URL)
            .await
            .context("failed to load CFX SSO page")?;
        let redirect_url: String = page
            .evaluate("JSON.parse(document.body.innerText).url")
            .await
            .context("failed to read CFX SSO redirect")?
            .into_value()
            .context("CFX SSO redirect was not a string")?;

        let cookie = CookieParam::builder()
            .name("_t")
            .value(forum_cookie)
            .domain("forum.cfx.re")
            .path("/")
            .http_only(true)
            .secure(true)
            .build()
            .map_err(|error| anyhow!("failed to build forum cookie: {error}"))?;
        self.browser
            .set_cookies(vec![cookie])
            .await
            .context("failed to set forum cookie")?;

        page.goto(&redirect_url)
            .await
            .context("failed to load CFX login redirect")?;
        wait_for_portal(&page).await?;

        page.goto(CREATED_ASSETS_URL)
            .await
            .context("failed to load created assets page")?;
        let url = page
            .url()
            .await
            .context("failed to read page URL")?
            .unwrap_or_default();
        if url.contains("login") {
            bail!("Redirected to login - cookie is expired or invalid");
        }

        Ok(())
    }

    pub(crate) async fn login(&self, forum_cookie: &str) -> Result<String> {
        self.authenticate(forum_cookie).await?;
        let cookies = self
            .browser
            .get_cookies()
            .await
            .context("failed to read browser cookies")?;

        Ok(cookies
            .iter()
            .map(|cookie| format!("{}={}", cookie.name, cookie.value))
            .collect::<Vec<_>>()
            .join("; "))
    }

    pub(crate) async fn close(mut self) -> Result<()> {
        let result = self
            .browser
            .close()
            .await
            .context("failed to close browser");
        if result.is_ok() {
            let _ = self.browser.wait().await;
        }
        self.handler.abort();
        let _ = self.handler.await;
        result.map(|_| ())
    }
}

async fn wait_for_portal(page: &Page) -> Result<()> {
    let deadline = Instant::now() + LOGIN_TIMEOUT;

    loop {
        if let Ok(result) = page
            .evaluate(
                "window.location.href.includes('portal.cfx.re') && \
                 !window.location.href.includes('/authenticate')",
            )
            .await
            && result.into_value::<bool>().unwrap_or(false)
        {
            return Ok(());
        }

        if Instant::now() >= deadline {
            bail!("Timed out waiting for CFX portal login");
        }
        sleep(POLL_INTERVAL).await;
    }
}
