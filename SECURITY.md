# Politique de sécurité — CreativeSuiteOpen

CreativeSuiteOpen prend la sécurité au sérieux, y compris pour une suite
créative open source. Ce document décrit comment signaler une vulnérabilité.

## Signaler une vulnérabilité

**Ne pas ouvrir d'issue publique pour signaler une vulnérabilité** — utilisez de préférence GitHub :

- Ouvrez un *security advisory* privé :
  https://github.com/vabyz971/CreativeSuiteOpen/security/advisories
- Ou écrivez à l'adresse du responsable via GitHub (l'issue discussion est
  acceptée pour les cas non sensibles).

Nous accusons réception sous 72 h et traitons les rapports vérifiés avec
priorité. Merci de ne pas publier de détails exploitables avant qu'un fix ne
soit disponible.

## Surfaces considérées sensibles

- Décodage d'images (PNG/JPEG) à l'ouverture et à l'export — dépend de
  `image` et de ses décodeurs.
- Parsing du format projet `.csophoto` (JSON versionné + base64) dans
  `photo-engine` (`project.rs`).
- Rendu GPU (shaders wgpu) — tout usage de mémoire GPU non maîtrisée y est
  rapporté comme une vulnérabilité.

## Versions supportées

Il n'existe pas encore de cycle de release formel : les correctifs de sécurité
sont publiés sur la branche `main` dès leur validation.