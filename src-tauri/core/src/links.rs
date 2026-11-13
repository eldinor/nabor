use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteLink {
    pub id: String,
    pub url: String,
    pub title: String,
    pub notes: String,
}

// These links are opened only on an explicit user action. Private/intranet
// addresses are allowed; no DNS lookup or HTTP request happens on save/open.
pub fn website_url(value: &str) -> Result<String> {
    let text = value.trim();
    if text.len() > 4096 || text.chars().any(char::is_control) {
        return Err("Website URL is too long or contains control characters".into());
    }
    let u = url::Url::parse(text).map_err(|_| "Enter a full http:// or https:// URL")?;
    if !matches!(u.scheme(), "http" | "https") || u.host_str().is_none() {
        return Err("Only http:// and https:// website links are supported".into());
    }
    if !u.username().is_empty() || u.password().is_some() {
        return Err("Remove the username and password from the website URL".into());
    }
    Ok(u.to_string())
}
pub(crate) fn validate_links(links: &[WebsiteLink]) -> Result<()> {
    if links.len() > 50 || serde_json::to_vec(links).map_err(err)?.len() > 64_000 {
        return Err(
            "An item supports up to 50 website links and 64,000 bytes of link details".into(),
        );
    }
    let mut ids = std::collections::HashSet::new();
    for link in links {
        valid_id(&link.id)?;
        if !ids.insert(&link.id) {
            return Err("Duplicate website link ID".into());
        }
        website_url(&link.url)?;
        if link.title.len() > 512 || link.notes.len() > 8000 {
            return Err("Link title exceeds 512 bytes or notes exceed 8,000 bytes".into());
        }
    }
    Ok(())
}
impl Item {
    pub fn website_links_context(&self) -> String {
        if self.links.is_empty() {
            return String::new();
        }
        format!("Website reference attachments (URLs and user notes only; website contents have NOT been fetched):\n{}",serde_json::to_string(&self.links).unwrap_or_default())
    }
    pub fn agent_reference_notes(&self) -> String {
        if self.links.is_empty() {
            self.notes.clone()
        } else {
            format!("{}\n\n{}", self.notes, self.website_links_context())
        }
    }
}
impl Collection {
    pub fn save_website_link(
        &self,
        item_id: &str,
        expected_hash: &str,
        link_id: Option<&str>,
        url: String,
        title: String,
        notes: String,
    ) -> Result<Item> {
        let mut item = self.load(item_id)?;
        if item.source_hash != expected_hash {
            return Err("This item changed. Reopen it before saving the link.".into());
        }
        let link = WebsiteLink {
            id: link_id.map(str::to_owned).unwrap_or_else(id),
            url: website_url(&url)?,
            title: title.trim().into(),
            notes,
        };
        if let Some(link_id) = link_id {
            let existing = item
                .links
                .iter_mut()
                .find(|l| l.id == link_id)
                .ok_or("Website link not found")?;
            *existing = link;
        } else {
            item.links.push(link);
        }
        validate_links(&item.links)?;
        self.save_new_revision(&mut item)?;
        Ok(item)
    }
    pub fn remove_website_link(
        &self,
        item_id: &str,
        expected_hash: &str,
        link_id: &str,
    ) -> Result<Item> {
        let mut item = self.load(item_id)?;
        if item.source_hash != expected_hash {
            return Err("This item changed. Reopen it before removing the link.".into());
        }
        let index = item
            .links
            .iter()
            .position(|l| l.id == link_id)
            .ok_or("Website link not found")?;
        item.links.remove(index);
        self.save_new_revision(&mut item)?;
        Ok(item)
    }
    pub fn website_link_url(&self, item_id: &str, link_id: &str) -> Result<String> {
        let item = self.load(item_id)?;
        let link = item
            .links
            .iter()
            .find(|l| l.id == link_id)
            .ok_or("Website link not found")?;
        website_url(&link.url)
    }
}
