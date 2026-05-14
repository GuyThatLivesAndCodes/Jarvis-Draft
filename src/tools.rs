use serde::{Deserialize, Serialize};
use reqwest::Client;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
}

pub fn get_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get current weather for a location".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "City name or coordinates (e.g., 'San Francisco' or '37.77,-122.42')"
                    }
                },
                "required": ["location"]
            }),
        },
        ToolDefinition {
            name: "move_jarvis".to_string(),
            description: "Move Jarvis (the central glowing orb) to a new position on the screen. Use this when the user asks you to move yourself or move the Jarvis orb.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "position": {
                        "type": "string",
                        "enum": ["center", "top-left", "top-center", "top-right", "middle-left", "middle-right", "bottom-left", "bottom-center", "bottom-right"],
                        "description": "Where to move Jarvis on the screen"
                    }
                },
                "required": ["position"]
            }),
        },
        ToolDefinition {
            name: "show_location".to_string(),
            description: "Display a location on Google Maps".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "Location name, address, or coordinates"
                    },
                    "zoom": {
                        "type": "integer",
                        "description": "Zoom level (1-20)"
                    }
                },
                "required": ["location"]
            }),
        },
    ]
}

pub async fn execute_tool(name: &str, input: serde_json::Value) -> Result<String, String> {
    match name {
        "get_weather" => get_weather(&input).await,
        "move_jarvis" => move_jarvis(&input),
        "show_location" => show_location(&input),
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

async fn get_weather(input: &serde_json::Value) -> Result<String, String> {
    let location = input
        .get("location")
        .and_then(|v| v.as_str())
        .ok_or("Missing location parameter")?;

    // Using Open-Meteo API (free, no key required)
    let client = Client::new();

    // Geocode the location first
    let geo_url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en&format=json",
        urlencoding::encode(location)
    );

    let geo_resp: serde_json::Value = client
        .get(&geo_url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let results = geo_resp
        .get("results")
        .and_then(|r| r.as_array())
        .ok_or("Location not found")?;

    if results.is_empty() {
        return Err("Location not found".to_string());
    }

    let lat = results[0]
        .get("latitude")
        .and_then(|v| v.as_f64())
        .ok_or("Invalid location data")?;
    let lon = results[0]
        .get("longitude")
        .and_then(|v| v.as_f64())
        .ok_or("Invalid location data")?;
    let place_name = results[0]
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(location);

    // Get weather data
    let weather_url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,relative_humidity_2m,weather_code,wind_speed_10m&temperature_unit=fahrenheit",
        lat, lon
    );

    let weather_resp: serde_json::Value = client
        .get(&weather_url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let current = weather_resp
        .get("current")
        .ok_or("Failed to get weather data")?;

    let temp = current
        .get("temperature_2m")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let humidity = current
        .get("relative_humidity_2m")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let wind = current
        .get("wind_speed_10m")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    Ok(format!(
        "Weather in {}: {}°F, Humidity: {}%, Wind: {} mph",
        place_name, temp, humidity, wind
    ))
}

fn move_jarvis(input: &serde_json::Value) -> Result<String, String> {
    let position = input
        .get("position")
        .and_then(|v| v.as_str())
        .ok_or("Missing position parameter")?;

    Ok(format!("Moved to {}.", position))
}

fn show_location(input: &serde_json::Value) -> Result<String, String> {
    let location = input
        .get("location")
        .and_then(|v| v.as_str())
        .ok_or("Missing location parameter")?;

    let zoom = input
        .get("zoom")
        .and_then(|v| v.as_i64())
        .unwrap_or(15);

    let maps_url = format!(
        "https://www.google.com/maps/search/{}/@0,0,{}z",
        urlencoding::encode(location),
        zoom
    );

    Ok(format!(
        "Opening map for '{}'. View it here: {}",
        location, maps_url
    ))
}
