//! La température du GPU, que `hwmon` ne donne pas.
//!
//! Le pilote propriétaire NVIDIA **n'enregistre aucun `hwmon`** : sur SHYNAEL,
//! le seul `hwmon` de carte graphique est celui du GPU intégré au Ryzen
//! (`amdgpu`). La RTX 5070 ne s'y trouve pas, et c'est la seule sonde que
//! l'utilisateur regarde vraiment qui manque.
//!
//! Elle se lit donc par `nvidia-smi`, mesuré à **16 ms** — un tiers d'image de
//! rendu. C'est pourquoi le démon l'appelle depuis un fil à part et met le
//! résultat en cache : jamais dans la boucle qui écrit sur les bus.
//!
//! Aucune dépendance : ni NVML, ni bibliothèque système à lier. Sans
//! `nvidia-smi`, la sonde est simplement absente — ce que le reste doit savoir
//! encaisser, une machine sans carte NVIDIA étant le cas ordinaire ailleurs.

/// La sonde du GPU NVIDIA : son nom et sa température en millidegrés.
///
/// `None` quand `nvidia-smi` est absent, échoue, ou rend autre chose qu'un
/// nombre — jamais une valeur inventée.
pub fn nvidia() -> Option<(String, i32)> {
    todo!("issue #31")
}
