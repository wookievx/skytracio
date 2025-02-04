use std::{collections::HashMap, fmt::Debug, fs, io, path::PathBuf, sync::{Arc, RwLock}};

use bevy::{log::{error, info}, prelude::Resource};
use ureq::serde_json;

//need to wrap in ARC
pub type OrbitalData = Vec<Arc<sgp4::Elements>>;

#[async_trait::async_trait]
pub trait EpochDataLoader {
    type Error: Debug;
    async fn load(&self, group: String, format: String) -> Result<OrbitalData, Self::Error>;
    async fn load_or_empty(&self, group: String, format: String) -> OrbitalData {
        self.load(group.clone(), format.clone()).await.unwrap_or_else(|er| {
            error!("Failed to load {group}&{format}, {er:?}");
            vec![]
        })
    }
}

#[derive(Clone, Resource)]
pub struct DefaultClient {
    cache: Arc<RwLock<HashMap<(String, String), OrbitalData>>>
}

impl DefaultClient {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::default()))
        }
    }
}

#[async_trait::async_trait]
impl EpochDataLoader for DefaultClient {
    type Error = ureq::Error;

    async fn load(&self, group: String, format: String) -> Result<OrbitalData, Self::Error> {
        info!("Calling API");
        if let Some(data) = self.cache
            .read()
            .unwrap()
            .get(&(group.clone(), format.clone())) {
            Ok(data.clone())
        } else {
            let mut guard = self.cache.write().unwrap();
            guard.insert((group.clone(), format.clone()), vec![]);
            
            let response = ureq::get("https://celestrak.com/NORAD/elements/gp.php")
                .query("GROUP", &group)
                .query("FORMAT", &format)
                .call()?;
            let elements_vec: Vec<sgp4::Elements> = response.into_json()?;
            let elements_vec: Vec<_> = elements_vec.into_iter().map(|el| Arc::new(el)).collect();

            let mut guard = self.cache.write().unwrap();
            guard.insert((group.clone(), format.clone()), elements_vec.clone());
            Ok(elements_vec)
        }
   
    }
}

#[derive(Clone, Debug, Resource)]
pub struct ConstFileClient {
    top_path: PathBuf
}

impl ConstFileClient {
    pub fn new(top_path: PathBuf) -> Self {
        Self { top_path }
    }
}

#[derive(Debug)]
pub enum ConstFileError {
    IO(io::Error),
    Serde(serde_json::Error)
}

impl From<io::Error> for ConstFileError {
    fn from(value: io::Error) -> Self {
        Self::IO(value)
    }
}

impl From<serde_json::Error> for ConstFileError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serde(value)
    }
}

#[async_trait::async_trait]
impl EpochDataLoader for ConstFileClient {
    type Error = ConstFileError;

    async fn load(&self, group: String, format: String) -> Result<OrbitalData, Self::Error>  {
        let extension = if format.as_str() == "JSON" {
            "json"
        } else {
            unimplemented!("Not supporting format: {}", format)
        };

        let mut path = self.top_path.clone();
        path.push("data");
        path.push(format!("{}.{}", group, extension));
        let file = fs::File::open(path)?;
        let data: Vec<sgp4::Elements> = serde_json::from_reader(file)?;
        let data: Vec<_> = data.into_iter().map(|el| Arc::new(el)).collect();
        Ok(data)
    }
}




#[cfg(test)]
mod tests {

    use super::*;
    use bevy::tasks::futures_lite::future::block_on;
    use sgp4::Elements;

    #[test]
    fn test_integration() {

        let client = DefaultClient::new();

        let res = block_on(client.load("galileo".to_owned(), "json".to_owned())).unwrap();

        println!("{}", display_elements(&res));
        assert!(res.len() > 1);        
    }

    fn display_elements(elements: &Vec<Arc<Elements>>) -> String {
        let res: Vec<_> = elements.iter().map(|els| format!("object_name={:?},international_designator={:?},norad_id={},classification={:?},datetime={:?}", els.object_name, els.international_designator, els.norad_id, display_clasification(&els), els.datetime)).collect();
        res.join("\n")
    }

    fn display_clasification(elem: &Elements) -> String {
        match elem.classification {
            sgp4::Classification::Unclassified => "unclassified".to_owned(),
            sgp4::Classification::Classified => "classified".to_owned(),
            sgp4::Classification::Secret => "secret".to_owned(),
        }
    }
}