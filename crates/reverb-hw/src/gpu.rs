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
    let sortie = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=name,temperature.gpu", "--format=csv,noheader"])
        .output()
        .ok()?;
    if !sortie.status.success() {
        return None;
    }
    depuis_nvidia_smi(&String::from_utf8_lossy(&sortie.stdout))
}

/// Découpe une ligne de `nvidia-smi`, séparée du lancement pour être vérifiable.
///
/// La première carte seulement : le cadran du Kraken n'en montre qu'une, et
/// deviner laquelle sur une machine à deux cartes serait une décision qu'aucune
/// donnée ne porte.
pub fn depuis_nvidia_smi(sortie: &str) -> Option<(String, i32)> {
    let ligne = sortie.lines().next()?;
    let (nom, degres) = ligne.rsplit_once(',')?;
    let nom = nom.trim();
    // `[N/A]` et `[Not Supported]` sont les réponses d'une carte qui ne rend pas
    // sa température : ce ne sont pas des nombres, et il ne faut pas en inventer.
    let degres: i32 = degres.trim().parse().ok()?;
    // Les espaces deviennent des blancs soulignés : le protocole du socket
    // sépare ses champs par des espaces, et « NVIDIA GeForce RTX 5070 » y
    // passerait pour quatre champs.
    let nom: String = nom
        .chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect();
    (!nom.is_empty()).then_some((nom, degres * 1000))
}
