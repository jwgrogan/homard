use crate::types::ToolSchema;
use crate::error::{HomardError, Result};

pub fn search_schema() -> ToolSchema {
    ToolSchema {
        name: "web_search".to_string(),
        description: "Search the web for information".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" }
            },
            "required": ["query"]
        }),
    }
}

pub fn fetch_schema() -> ToolSchema {
    ToolSchema {
        name: "web_fetch".to_string(),
        description: "Fetch content from a URL".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "URL to fetch" }
            },
            "required": ["url"]
        }),
    }
}

pub async fn search(args: serde_json::Value) -> Result<String> {
    let query = args.get("query")
        .and_then(|q| q.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'query' argument".to_string()))?;

    // Use DuckDuckGo HTML for simplicity (no API key needed)
    let url = format!("https://html.duckduckgo.com/html/?q={}", urlencoding::encode(query));
    let client = reqwest::Client::new();
    let resp = client.get(&url)
        .header("User-Agent", "Homard/1.0")
        .send()
        .await
        .map_err(|e| HomardError::Tool(e.to_string()))?;

    let body = resp.text().await.map_err(|e| HomardError::Tool(e.to_string()))?;

    // Extract result snippets (basic HTML parsing)
    let mut results = Vec::new();
    for line in body.lines() {
        if line.contains("result__snippet") {
            let clean = line.replace("<b>", "").replace("</b>", "")
                .replace("&quot;", "\"").replace("&amp;", "&");
            // Strip HTML tags
            let text: String = clean.chars().fold((String::new(), false), |(mut acc, in_tag), c| {
                if c == '<' { (acc, true) }
                else if c == '>' { (acc, false) }
                else if !in_tag { acc.push(c); (acc, false) }
                else { (acc, true) }
            }).0;
            let trimmed = text.trim().to_string();
            if !trimmed.is_empty() {
                results.push(trimmed);
            }
        }
        if results.len() >= 5 { break; }
    }

    if results.is_empty() {
        Ok("No results found.".to_string())
    } else {
        Ok(results.join("\n\n"))
    }
}

pub async fn fetch(args: serde_json::Value) -> Result<String> {
    let url = args.get("url")
        .and_then(|u| u.as_str())
        .ok_or_else(|| HomardError::Tool("Missing 'url' argument".to_string()))?;

    let client = reqwest::Client::new();
    let resp = client.get(url)
        .header("User-Agent", "Homard/1.0")
        .send()
        .await
        .map_err(|e| HomardError::Tool(e.to_string()))?;

    let body = resp.text().await.map_err(|e| HomardError::Tool(e.to_string()))?;

    // Basic HTML to text (strip tags)
    let text: String = body.chars().fold((String::new(), false), |(mut acc, in_tag), c| {
        if c == '<' { (acc, true) }
        else if c == '>' { (acc, false) }
        else if !in_tag { acc.push(c); (acc, false) }
        else { (acc, true) }
    }).0;

    Ok(text.trim().to_string())
}
