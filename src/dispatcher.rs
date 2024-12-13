use crate::*;
use reqwest::Error as ReqwestError;
use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderMap, CONTENT_TYPE};
use serde::{Serialize, Deserialize};


const DEFAULT_DISPATCHER_URL: &str = "https://dispatcher-api.psi.ch/sf";
const BASE_URL: &str = DEFAULT_DISPATCHER_URL;


#[derive(Serialize, Deserialize, Debug)]
pub struct ChannelDescription {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    modulo: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    offset: Option<u32>,
}

impl ChannelDescription {
    pub fn new(name: &str, modulo: u32, offset: u32) -> Self {
        Self{name: name.to_string(), modulo:Some(modulo), offset:Some(offset)}
    }
    pub fn of(name: &str) -> Self {
        Self{name: name.to_string(),modulo:None, offset:None}
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    channels: Vec<ChannelDescription>,
    stream_type: String,
    verify: bool,
    channel_validation: ChannelValidation,
    #[serde(skip_serializing_if = "Option::is_none")]
    compression: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ChannelValidation {
    inconsistency: String,
}

pub struct DispatcherStream {
    endpoint: String,
}

impl DispatcherStream{
    pub fn get_endpoint(&self) -> &str {
        self.endpoint.as_str()
    }
}

impl Drop for DispatcherStream {
    fn drop(& mut self) {
        match remove_stream(self.endpoint.as_str()) {
            Ok(_) => {}
            Err(e) => {println!("Error removing stream: {}", e)}
        }
    }
}

pub fn request_stream(channels: Vec<ChannelDescription>, stream_type: Option<String>, inconsistency_resolution: Option<String>,
                      verify: bool,disable_compression: bool,) -> IOResult<DispatcherStream> {
    let stream_type = stream_type.unwrap_or_else(|| "pub_sub".to_string());
    let inconsistency_resolution = inconsistency_resolution.unwrap_or_else(|| {
        if verify {
            "adjust-individual".to_string()
        } else {
            "keep-as-is".to_string()
        }
    });

    let mut config = Config {
        channels,
        stream_type,
        verify,
        channel_validation: ChannelValidation {
            inconsistency: inconsistency_resolution,
        },
        compression: None,
    };

    if disable_compression {
        config.compression = Some("none".to_string());
    }

    let client = Client::new();
    let url = format!("{}/stream", BASE_URL);

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());

    let response: Response = client
        .post(&url)
        .headers(headers)
        .json(&config)
        .send()
        .map_err(|e: ReqwestError|new_error(ErrorKind::ConnectionRefused, e.to_string().as_str()))?;

    if !response.status().is_success() {
        let error_msg = match response.text(){
            Ok(msg) => { format!("Unable to request stream {:?}: {}", config, msg)}
            Err(err) => {format!("Error requesting stream {:?}: {}", config, err.to_string())}
        };
        return Err( new_error (ErrorKind::Other, error_msg.as_str()));
    }

    let json: serde_json::Value = response.json().map_err(|e: ReqwestError|new_error(ErrorKind::InvalidData, e.to_string().as_str()))?;
    let endpoint = json["stream"].as_str().unwrap().to_string();
    println!("Created stream: {}", endpoint);
    Ok(DispatcherStream{endpoint})
}


fn remove_stream(stream: &str) -> IOResult<()> {
    println!("Removing stream: {}", stream);
    let client = Client::new();
    let url = format!("{}/stream", BASE_URL);

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::CONTENT_TYPE, "text/plain".parse().unwrap());

    let response = client
        .delete(&url)
        .headers(headers)
        .body(stream.to_string())  // Send the stream as the body
        .send()
        .map_err(|e: ReqwestError| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    if !response.status().is_success() {
        let error_msg = match response.text(){
            Ok(msg) => { format!("Unable to delete stream {}: {}", stream, msg)}
            Err(err) => {format!("Error deleting stream {}: {}", stream, err.to_string())}
        };
    }
    Ok(())
}
