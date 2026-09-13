// Cygnus — Suite créative professionnelle open source
// Copyright (C) 2026 vabyz971
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Transformation affine 2D canonique (convention scale → skew → rotation
//! autour du centre, puis décalage).
//!
//! Source UNIQUE de la formule : `photo-engine` (compositing) et `ui-kit`
//! (`image_canvas::corners`) l'appellent — aucune copie locale ailleurs.

use serde::{Deserialize, Serialize};

/// Transformation affine appliquée AU DRAW (modèle « state-only » : changer
/// une valeur ne régénère jamais les pixels). Convention d'ordre des
/// opérations (autour du centre de l'image) :
/// scale → skew → rotation → décalage. `skew_x` cisaille X selon Y (le bord
/// droit en X), `skew_y` cisaille Y selon X, en degrés.
///
/// Sérialisation rétro-compatible : les projets anciens (champ uniforme
/// `scale`) sont lus via [`Deserialize`] custom — `scale` devient
/// `scale_x == scale_y`. Les nouveaux champs ont leurs défauts dans les JSON
/// absents (Deserialize derive accepte les champs manquants).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Transform2D {
    pub offset_x: f32,
    pub offset_y: f32,
    /// Rotation en degrés (sens horaire), appliquée autour du centre
    pub rotation_deg: f32,
    /// Échelle X de l'image (1.0 = 100 %)
    pub scale_x: f32,
    /// Échelle Y de l'image (1.0 = 100 %)
    pub scale_y: f32,
    /// Inclinaison horizontale en degrés (cisaillement de X selon Y)
    pub skew_x: f32,
    /// Inclinaison verticale en degrés (cisaillement de Y selon X)
    pub skew_y: f32,
}

impl Default for Transform2D {
    /// Transformation identité.
    fn default() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            rotation_deg: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            skew_x: 0.0,
            skew_y: 0.0,
        }
    }
}

impl<'de> Deserialize<'de> for Transform2D {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            OffsetX,
            OffsetY,
            RotationDeg,
            #[serde(rename = "scale")]
            ScaleLegacy,
            ScaleX,
            ScaleY,
            SkewX,
            SkewY,
        }

        struct Transform2DVisitor;

        impl<'de> serde::de::Visitor<'de> for Transform2DVisitor {
            type Value = Transform2D;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("struct Transform2D")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut t = Transform2D::default();
                let mut has_sx = false;
                let mut has_sy = false;
                let mut legacy_scale: Option<f32> = None;
                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::OffsetX => t.offset_x = map.next_value()?,
                        Field::OffsetY => t.offset_y = map.next_value()?,
                        Field::RotationDeg => t.rotation_deg = map.next_value()?,
                        Field::ScaleX => {
                            t.scale_x = map.next_value()?;
                            has_sx = true;
                        }
                        Field::ScaleY => {
                            t.scale_y = map.next_value()?;
                            has_sy = true;
                        }
                        Field::SkewX => t.skew_x = map.next_value()?,
                        Field::SkewY => t.skew_y = map.next_value()?,
                        Field::ScaleLegacy => legacy_scale = Some(map.next_value::<f32>()?),
                    }
                }
                // Projet ancien (`scale` uniforme) : propager vers les axes sauf
                // si une échelle par axe était déjà explicitement lue.
                if let Some(s) = legacy_scale {
                    if !has_sx {
                        t.scale_x = s;
                    }
                    if !has_sy {
                        t.scale_y = s;
                    }
                }
                Ok(t)
            }
        }

        deserializer.deserialize_map(Transform2DVisitor)
    }
}

impl Transform2D {
    /// Point local d'image (px, origine coin supérieur gauche) → doc
    /// (coordonnées canvas, centre doc = (doc_w/2, doc_h/2)). Même
    /// convention que le draw du canvas : l'offset est le coin
    /// supérieur-gauche du rectangle scalé (avant rotation) et
    /// cisaillement/rotation se font autour du centre du rectangle scalé.
    #[must_use]
    pub fn local_to_doc(&self, w0: f32, h0: f32, x: f32, y: f32) -> (f32, f32) {
        let sx = self.scale_x;
        let sy = self.scale_y;
        let kx = self.skew_x.to_radians().tan();
        let ky = self.skew_y.to_radians().tan();
        let cx = w0 / 2.0;
        let cy = h0 / 2.0;
        let ux = (x - cx) * sx;
        let uy = (y - cy) * sy;
        let tx = ux + kx * uy;
        let ty = ky * ux + uy;
        let rad = self.rotation_deg.to_radians();
        let (cos, sin) = (rad.cos(), rad.sin());
        (
            tx * cos - ty * sin + cx * sx + self.offset_x,
            tx * sin + ty * cos + cy * sy + self.offset_y,
        )
    }

    /// Les 4 coins de l'image (tl, tr, br, bl) en coordonnées doc.
    #[must_use]
    pub fn doc_corners(&self, w0: f32, h0: f32) -> [(f32, f32); 4] {
        [(0.0, 0.0), (w0, 0.0), (w0, h0), (0.0, h0)].map(|(x, y)| self.local_to_doc(w0, h0, x, y))
    }

    /// (m00, m01, m10, m11) — la matrice cisaillement×échelle, AVANT rotation.
    /// Utilisée à la fois par le rendu (compositing) et par tout futur
    /// calcul de coins/poignées. SEUL endroit où `tan()` est appliqué.
    #[must_use]
    pub fn shear_scale_matrix(&self) -> (f32, f32, f32, f32) {
        let kx = self.skew_x.to_radians().tan();
        let ky = self.skew_y.to_radians().tan();
        (
            self.scale_x,
            kx * self.scale_y,
            ky * self.scale_x,
            self.scale_y,
        )
    }

    /// Vrai si une inclinaison est active (le chemin GPU rapide ne sait pas
    /// l'afficher : on passera par le compositing CPU).
    #[must_use]
    pub fn has_skew(&self) -> bool {
        self.skew_x.abs() > 0.001 || self.skew_y.abs() > 0.001
    }
}
