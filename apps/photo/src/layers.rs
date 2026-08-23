//! Façade app vers le module document du moteur photo.
//! Le modèle de calques vit dans photo-engine (réutilisables par les
//! autres apps de la suite) ; l'affichage passe par le canvas GPU.

pub use photo_engine::document::{Layer, BLEND_MODES};
