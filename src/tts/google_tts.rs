use std::env;
use base64_stream::FromBase64Reader;
use std::io::Cursor;
use std::io::Read;
use super::models::*;

pub async fn message_to_speech(msg: &String) -> Result<Vec<u8>, reqwest::Error> {
    let body = VoiceRequest {
        input: VoiceInput {
            text: msg.to_string(),
        },
        voice: Voice {
            language_code: "en-US".to_string(),
            name: "en-US-Wavenet-A".to_string(),
            ssml_gender: "FEMALE".to_string(),
        },
        audio_config: AudioConfig {
            audio_encoding: "OGG_OPUS".to_string()
        }
    };
    let client = reqwest::Client::new();
    let key = env::var("GOOGLE_API_KEY").expect("Expected a token in the environment");
    let url = format!("https://texttospeech.googleapis.com/v1/text:synthesize?key={}", key);
    let res = client.post(&url)
        .json(&body)
        .send()
        .await?
        .json::<VoiceResponse>().await?;
    let mut reader = FromBase64Reader::new(Cursor::new(res.audio_content));
    let mut buff = Vec::new();
    reader.read_to_end(&mut buff).unwrap();
    Ok(buff)
}
