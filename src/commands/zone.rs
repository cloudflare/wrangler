use crate::http;
use crate::terminal::styles;

use crate::settings::global_user::GlobalUser;

use cloudflare::endpoints::zone::{ListZones, ListZonesParams};
use cloudflare::framework::apiclient::ApiClient;

use anyhow::Result;

const ERR_MESSAGE: &str = "No matching zones found. Check you entered the zone name correctly and that Wrangler has permission to read zone information in your Cloudflare accounts.";

pub fn zone(user: &GlobalUser, zone: String) -> Result<()> {
    let api_client = http::cf_v4_client(user)?;

    let resp = api_client.request(&ListZones {
        params: ListZonesParams {
            name: Some(zone),
            status: None,
            page: None,
            per_page: Some(50),
            order: None,
            direction: None,
            search_match: None,
        },
    })?;

    if resp.result.is_empty() {
        println!("{}", styles::warning(ERR_MESSAGE));
        return Ok(());
    }

    let zone = &resp.result[0];

    let status = format!("{:?}", zone.status);

    let title = format!("Zone information for {}", styles::url(&zone.name));

    let information = &[
        format!("{}\n", styles::bold(styles::underline(title))),
        format!("Zone ID: {}", styles::cyan(&zone.id)),
        format!(
            "Account: {} ({})",
            styles::cyan(&zone.account.name),
            styles::cyan(&zone.account.id)
        ),
        format!("Status: {}", styles::cyan(status)),
    ];

    for info_item in information {
        println!("{}", info_item);
    }

    Ok(())
}
