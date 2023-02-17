use std::borrow::BorrowMut;
use std::collections::BTreeMap;

use base64::DecodeError;
use base64::engine::general_purpose;
use lopdf::{Bookmark, Document, Object, ObjectId};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MergeRequest {
    files: Vec<String>,
}

impl MergeRequest {
    fn new(files: Vec<String>) -> Self {
        Self { files }
    }
}

fn merge(documents: Vec<Document>) -> Result<Document, String> {

    // If no "Pages" found abort
    if documents.is_empty() {
        return Err(String::from("empty file list"));
    }

    // This code comes from the getting started, I've just changed the signature
    // to return the Document instead of saving to a local file

    // Define a starting max_id (will be used as start index for object_ids)
    let mut max_id = 1;
    let mut pagenum = 1;
    // Collect all Documents Objects grouped by a map
    let mut documents_pages = BTreeMap::new();
    let mut documents_objects = BTreeMap::new();
    let mut document = Document::with_version("1.5");

    for mut doc in documents {
        let mut first = false;
        doc.renumber_objects_with(max_id);

        max_id = doc.max_id + 1;

        documents_pages.extend(
            doc
                .get_pages()
                .into_iter()
                .map(|(_, object_id)| {
                    if !first {
                        let bookmark = Bookmark::new(String::from(format!("Page_{}", pagenum)), [0.0, 0.0, 1.0], 0, object_id);
                        document.add_bookmark(bookmark, None);
                        first = true;
                        pagenum += 1;
                    }

                    (
                        object_id,
                        doc.get_object(object_id).unwrap().to_owned(),
                    )
                })
                .collect::<BTreeMap<ObjectId, Object>>(),
        );
        documents_objects.extend(doc.objects);
    }

    // Catalog and Pages are mandatory
    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    // Process all objects except "Page" type
    for (object_id, object) in documents_objects.iter() {
        // We have to ignore "Page" (as are processed later), "Outlines" and "Outline" objects
        // All other objects should be collected and inserted into the main Document
        match object.type_name().unwrap_or("") {
            "Catalog" => {
                // Collect a first "Catalog" object and use it for the future "Pages"
                catalog_object = Some((
                    if let Some((id, _)) = catalog_object {
                        id
                    } else {
                        *object_id
                    },
                    object.clone(),
                ));
            }
            "Pages" => {
                // Collect and update a first "Pages" object and use it for the future "Catalog"
                // We have also to merge all dictionaries of the old and the new "Pages" object
                if let Ok(dictionary) = object.as_dict() {
                    let mut dictionary = dictionary.clone();
                    if let Some((_, ref object)) = pages_object {
                        if let Ok(old_dictionary) = object.as_dict() {
                            dictionary.extend(old_dictionary);
                        }
                    }

                    pages_object = Some((
                        if let Some((id, _)) = pages_object {
                            id
                        } else {
                            *object_id
                        },
                        Object::Dictionary(dictionary),
                    ));
                }
            }
            "Page" => {}     // Ignored, processed later and separately
            "Outlines" => {} // Ignored, not supported yet
            "Outline" => {}  // Ignored, not supported yet
            _ => {
                document.objects.insert(*object_id, object.clone());
            }
        }
    }

    // If no "Pages" found abort
    if pages_object.is_none() {
        return Err(String::from("root page not found"));
    }

    // Iter over all "Page" and collect with the parent "Pages" created before
    for (object_id, object) in documents_pages.iter() {
        if let Ok(dictionary) = object.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Parent", pages_object.as_ref().unwrap().0);

            document
                .objects
                .insert(*object_id, Object::Dictionary(dictionary));
        }
    }

    // If no "Catalog" found abort
    if catalog_object.is_none() {
        return Err(String::from("root catalog not found"));
    }

    let catalog_object = catalog_object.unwrap();
    let pages_object = pages_object.unwrap();

    // Build a new "Pages" with updated fields
    if let Ok(dictionary) = pages_object.1.as_dict() {
        let mut dictionary = dictionary.clone();

        // Set new pages count
        dictionary.set("Count", documents_pages.len() as u32);

        // Set new "Kids" list (collected from documents pages) for "Pages"
        dictionary.set(
            "Kids",
            documents_pages
                .into_iter()
                .map(|(object_id, _)| Object::Reference(object_id))
                .collect::<Vec<_>>(),
        );

        document
            .objects
            .insert(pages_object.0, Object::Dictionary(dictionary));
    }

    // Build a new "Catalog" with updated fields
    if let Ok(dictionary) = catalog_object.1.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Pages", pages_object.0);
        dictionary.remove(b"Outlines"); // Outlines not supported in merged PDFs

        document
            .objects
            .insert(catalog_object.0, Object::Dictionary(dictionary));
    }

    document.trailer.set("Root", catalog_object.0);

    // Update the max internal ID as wasn't updated before due to direct objects insertion
    document.max_id = document.objects.len() as u32;

    // Reorder all new Document objects
    document.renumber_objects();

    //Set any Bookmarks to the First child if they are not set to a page
    document.adjust_zero_pages();

    //Set all bookmarks to the PDF Object tree then set the Outlines to the Bookmark content map.
    if let Some(n) = document.build_outline() {
        if let Ok(x) = document.get_object_mut(catalog_object.0) {
            if let Object::Dictionary(ref mut dict) = x {
                dict.set("Outlines", Object::Reference(n));
            }
        }
    }

    document.compress();
    Ok(document)
}

fn str_to_vec(input_b64: &str) -> Result<Vec<u8>, DecodeError> {
    use base64::Engine;
    general_purpose::STANDARD.decode(input_b64)
}

pub fn build(merge_request: MergeRequest) -> Result<Vec<u8>, String> {
    let mut merge_bytes = Vec::new();
    let merge_result: Result<Document, String>;

    let docs: Vec<Document> = merge_request.files.iter()
        .map(|s| str_to_vec(s.as_str()))
        .filter(|v| v.is_ok())
        .map(|s| Document::load_mem(s.unwrap().as_slice()))
        .filter(|v| v.is_ok())
        .map(|v| v.unwrap())
        .collect::<Vec<Document>>();

    merge_result = merge(docs);

    match merge_result {
        Ok(mut mr) => {
            mr.save_to(&mut merge_bytes);
            Ok(merge_bytes)
        }
        Err(er) => {
            Err(er)
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_merged_with_empty_list_input() {
        assert!(build(MergeRequest::default()).is_err());
    }

    #[test]
    fn get_merged_with_bad_list_input() {
        assert!(build(
            MergeRequest::new(vec![String::from("foo"), String::from("bar")])
        ).is_err());
    }

    #[test]
    fn get_merged_with_good_list_input() {
        let payload = include_str!("test/payload.json");
        let merge_request: MergeRequest = serde_json::from_str(payload).unwrap();
        assert!(build(merge_request).is_ok());
    }
}

