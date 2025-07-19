// Imports
use crate::document::{Format, background};
use crate::engine::import::{PdfImportPrefs, XoppImportPrefs};
use crate::fileformats::xoppformat::XoppBackgroundType;
use crate::fileformats::{FileFormatLoader, rnoteformat, xoppformat};
use crate::store::chrono_comp::StrokeLayer;
use crate::store::{ChronoComponent, StrokeKey};
use crate::strokes::Stroke;
use crate::strokes::VectorImage;
use crate::{Camera, Document, Engine};
use anyhow::Context;
use futures::channel::oneshot;
use serde::{Deserialize, Serialize};
use slotmap::{HopSlotMap, SecondaryMap};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::error;

/// Trait for types which hold configuration needed for engine snapshots
pub trait Snapshotable {
    fn extract_snapshot_data(&self) -> Self;
}

// An engine snapshot, used when loading/saving the current document from/into a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename = "engine_snapshot")]
pub struct EngineSnapshot {
    #[serde(rename = "document")]
    pub document: Document,
    #[serde(rename = "camera")]
    pub camera: Camera,
    #[serde(rename = "stroke_components")]
    pub stroke_components: Arc<HopSlotMap<StrokeKey, Arc<Stroke>>>,
    #[serde(rename = "chrono_components")]
    pub chrono_components: Arc<SecondaryMap<StrokeKey, Arc<ChronoComponent>>>,
    #[serde(rename = "chrono_counter")]
    pub chrono_counter: u32,
}

impl Default for EngineSnapshot {
    fn default() -> Self {
        Self {
            document: Document::default(),
            camera: Camera::default(),
            stroke_components: Arc::new(HopSlotMap::with_key()),
            chrono_components: Arc::new(SecondaryMap::new()),
            chrono_counter: 0,
        }
    }
}

impl EngineSnapshot {
    /// Loads a snapshot from the bytes of a .rnote file.
    ///
    /// To import this snapshot into the current engine, use [`Engine::load_snapshot()`].
    pub async fn load_from_rnote_bytes(bytes: Vec<u8>) -> anyhow::Result<Self> {
        let (snapshot_sender, snapshot_receiver) = oneshot::channel::<anyhow::Result<Self>>();

        rayon::spawn(move || {
            let result = || -> anyhow::Result<Self> {
                let rnote_file = rnoteformat::RnoteFile::load_from_bytes(&bytes)
                    .context("loading RnoteFile from bytes failed.")?;
                Ok(ijson::from_value(&rnote_file.engine_snapshot)?)
            };

            if let Err(_data) = snapshot_sender.send(result()) {
                error!(
                    "Sending bytes result to receiver failed while loading rnote bytes in. Receiver already dropped."
                );
            }
        });

        snapshot_receiver.await?
    }
    /// Loads from the bytes of a Xournal++ .xopp file.
    ///
    /// To import this snapshot into the current engine, use [`Engine::load_snapshot()`].
    pub async fn load_from_xopp_bytes(
        bytes: Vec<u8>,
        xopp_import_prefs: XoppImportPrefs,
        file_request_sender: oneshot::Sender<Option<Vec<String>>>,
        file_bytes_receiver: oneshot::Receiver<Option<HashMap<String, Vec<u8>>>>,
    ) -> anyhow::Result<Self> {
        let xopp_file = xoppformat::XoppFile::load_from_bytes(&bytes)?;

        // Extract the largest width of all pages, add together all heights
        let (doc_width, doc_height) = xopp_file
            .xopp_root
            .pages
            .iter()
            .map(|page| (page.width, page.height))
            .fold(
                (0_f64, 0_f64),
                |(prev_width, prev_height), (next_width, next_height)| {
                    (prev_width.max(next_width), prev_height + next_height)
                },
            );
        let no_pages = xopp_file.xopp_root.pages.len() as u32;

        // extract pdf info if it exist
        let files_to_request = {
            let files_found: Vec<String> = xopp_file
                .xopp_root
                .pages
                .iter()
                .map(|page| {
                    if let XoppBackgroundType::Pdf { filename, .. } = &page.background.bg_type {
                        filename.clone()
                    } else {
                        None
                    }
                })
                .filter(|filename| filename.is_some())
                .map(|filename| filename.unwrap())
                .collect();

            if files_found.len() == 0 {
                None
            } else {
                Some(files_found)
            }
        };

        let result_file_request = file_request_sender.send(files_to_request.clone());
        if result_file_request.is_err() && files_to_request.is_some() {
            return Err(anyhow::anyhow!(
                "Could not send the request to load linked files. Aborting import"
            ));
        }

        // wait for the response
        // This should always be sent even if no files are requested (as a None in that case)
        let file_bytes = file_bytes_receiver.await;
        let file_hash = if file_bytes.is_err() {
            return Err(anyhow::anyhow!("no response for the file requested"));
        } else {
            file_bytes.unwrap()
        };

        // verify we have all of the files
        match (files_to_request, file_hash.as_ref()) {
            (Some(_), None) => {
                return Err(anyhow::anyhow!(
                    "Expected to retrieve the linked files, found none"
                ));
            }
            (Some(file_vec), Some(hash)) => {
                if file_vec.len() != hash.len() {
                    return Err(anyhow::anyhow!(
                        "Error between the number of files requested and files given back"
                    ));
                }
            }
            _ => {}
        }

        let mut engine = Engine::default();

        // We convert all values from the hardcoded 72 DPI of Xopp files to the preferred dpi
        engine.document.config.format.set_dpi(xopp_import_prefs.dpi);

        engine.document.x = 0.0;
        engine.document.y = 0.0;
        engine.document.width = crate::utils::convert_value_dpi(
            doc_width,
            xoppformat::XoppFile::DPI,
            xopp_import_prefs.dpi,
        );
        engine.document.height = crate::utils::convert_value_dpi(
            doc_height,
            xoppformat::XoppFile::DPI,
            xopp_import_prefs.dpi,
        );

        engine
            .document
            .config
            .format
            .set_width(crate::utils::convert_value_dpi(
                doc_width,
                xoppformat::XoppFile::DPI,
                xopp_import_prefs.dpi,
            ));
        engine
            .document
            .config
            .format
            .set_height(crate::utils::convert_value_dpi(
                doc_height / (no_pages as f64),
                xoppformat::XoppFile::DPI,
                xopp_import_prefs.dpi,
            ));

        let mut pdf_filename: Option<String> = None;
        if let Some(first_page) = xopp_file.xopp_root.pages.first() {
            if let xoppformat::XoppBackgroundType::Solid {
                color: _color,
                style: _style,
            } = &first_page.background.bg_type
            {
                // Xopp background styles are not compatible with Rnotes, so everything is plain for now
                engine.document.config.background.pattern = background::PatternStyle::None;
            }
        }

        // Offsetting as rnote has one global coordinate space
        let mut offset = na::Vector2::<f64>::zeros();

        for page in xopp_file.xopp_root.pages.into_iter() {
            // background
            if let xoppformat::XoppBackgroundType::Pdf {
                pageno, filename, ..
            } = page.background.bg_type
            {
                if filename.is_some() {
                    pdf_filename = filename;
                }
                if let Some(hash_k) = &file_hash {
                    let pdf_bytes = hash_k.get(pdf_filename.as_ref().unwrap());

                    if pdf_bytes.is_some() {
                        let vec_img = VectorImage::from_pdf_bytes(
                            &pdf_bytes.unwrap(),
                            PdfImportPrefs {
                                page_width_perc: 100.0,
                                page_spacing:
                                    crate::engine::import::PdfImportPageSpacing::OnePerDocumentPage,
                                pages_type: crate::engine::import::PdfImportPagesType::Vector, // should we select from the engine here ?
                                adjust_document: false,
                                bitmap_scalefactor: 1.8,
                                page_borders: false,
                            },
                            offset,
                            Some(((pageno as u32) - 1)..(pageno as u32)),
                            &engine.document.config.format,
                            None, // for now no password given
                        );
                        match vec_img {
                            Ok(new_stroke) => {
                                let collection = new_stroke.iter().map(|element| {
                                    (
                                        Stroke::VectorImage(element.clone()), // not great
                                        Some(StrokeLayer::Document),
                                    )
                                });
                                for (el, layer) in collection {
                                    engine.store.insert_stroke(el, layer);
                                }
                            }
                            Err(e) => {
                                // todo: error msg
                            }
                        }
                    }
                }
            }

            for layers in page.layers.into_iter() {
                // import strokes
                for new_xoppstroke in layers.strokes.into_iter() {
                    match Stroke::from_xoppstroke(new_xoppstroke, offset, xopp_import_prefs.dpi) {
                        Ok((new_stroke, layer)) => {
                            engine.store.insert_stroke(new_stroke, Some(layer));
                        }
                        Err(e) => {
                            error!(
                                "Creating Stroke from XoppStroke failed while loading Xopp bytess, Err: {e:?}",
                            );
                        }
                    }
                }

                // import images
                for new_xoppimage in layers.images.into_iter() {
                    match Stroke::from_xoppimage(new_xoppimage, offset, xopp_import_prefs.dpi) {
                        Ok(new_image) => {
                            engine.store.insert_stroke(new_image, None);
                        }
                        Err(e) => {
                            error!(
                                "Creating Stroke from XoppImage failed while loading Xopp bytes, Err: {e:?}",
                            );
                        }
                    }
                }
            }

            // Only add to y offset, results in vertical pages
            offset[1] += crate::utils::convert_value_dpi(
                page.height,
                xoppformat::XoppFile::DPI,
                xopp_import_prefs.dpi,
            );
        }

        Ok(engine.take_snapshot())
    }
}
