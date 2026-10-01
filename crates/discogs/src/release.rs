use serde::Deserialize;

/// One search result, with only the fields dj-tui reads.
#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    /// Discogs writes these as "Artist - Title".
    #[serde(default)]
    pub title: String,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub genre: Vec<String>,
    /// Finer than `genre`: "Techno" where the genre says "Electronic". Null in some
    /// responses, which is read as an empty list rather than failing the parse.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub style: Vec<String>,
}

fn null_as_empty<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<Vec<String>>::deserialize(d)?.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    results: Vec<Release>,
}

impl Release {
    /// What dj-tui stores: the styles joined, the genre when there are no styles, or nothing
    /// at all. An empty string would be stored and would then filter as a real genre.
    pub fn genre(&self) -> Option<String> {
        for field in [&self.style, &self.genre] {
            let values: Vec<&str> = field
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if !values.is_empty() {
                return Some(values.join(", "));
            }
        }
        None
    }

    /// The artist and title halves of Discogs' "Artist - Title".
    fn parts(&self) -> (&str, &str) {
        match self.title.split_once(" - ") {
            Some((artist, title)) => (artist, title),
            None => ("", self.title.as_str()),
        }
    }
}

pub fn parse_search(json: &str) -> Result<Vec<Release>, serde_json::Error> {
    Ok(serde_json::from_str::<SearchResponse>(json)?.results)
}

/// Lowercase, drop everything that is not a letter or a digit, and cut a feature credit off
/// the end. Two names that survive this the same way are the same record.
fn normalise(text: &str) -> String {
    let text = text.to_lowercase();
    let text = ["feat.", "feat ", "ft.", "ft ", "featuring"]
        .iter()
        .fold(text, |acc, marker| match acc.split_once(marker) {
            Some((before, _)) => before.to_string(),
            None => acc,
        });
    text.chars().filter(|c| c.is_alphanumeric()).collect()
}

/// The release that is actually the record asked for, or nothing. A search returns something
/// whatever it is asked, so both halves have to agree before a genre is taken off it.
pub fn best_match<'a>(results: &'a [Release], artist: &str, title: &str) -> Option<&'a Release> {
    let want_artist = normalise(artist);
    let want_title = normalise(title);
    if want_artist.is_empty() || want_title.is_empty() {
        return None;
    }
    results.iter().find(|release| {
        let (their_artist, their_title) = release.parts();
        let their_artist = normalise(their_artist);
        let their_title = normalise(their_title);
        if their_artist.is_empty() || their_title.is_empty() {
            return false;
        }
        // One side may carry a remix credit or an A/B pairing, so containment either way
        // counts, but both halves have to agree.
        let artist_ok = their_artist.contains(&want_artist) || want_artist.contains(&their_artist);
        let title_ok = their_title.contains(&want_title) || want_title.contains(&their_title);
        artist_ok && title_ok
    })
}
