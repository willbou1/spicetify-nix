use octocrab::{models::Repository, Octocrab};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Blacklist {
    repos: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Author {
    name: Option<String>,
    email: Option<String>,
    url: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ExtManifest {
    name: Option<String>,
    description: Option<String>,
    preview: Option<String>,
    main: Option<String>,
    readme: Option<String>,
    branch: Option<String>,
    authors: Option<Vec<Author>>,
    tags: Option<Vec<String>>,
}
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ListOrExtManifest {
    Not(ExtManifest),
    List(Vec<ExtManifest>),
}

#[derive(Serialize, Deserialize)]
struct ThemeManifest {
    name: Option<String>,
    description: Option<String>,
    preview: Option<String>,
    usercss: Option<String>,
    schemes: Option<String>,
    include: Option<Vec<String>>,
    readme: Option<String>,
    branch: Option<String>,
    authors: Option<Vec<Author>>,
    tags: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ListOrThemeManifest {
    Not(ThemeManifest),
    List(Vec<ThemeManifest>),
}

#[derive(Serialize, Deserialize)]
struct AppManifest {
    name: Option<String>,
    description: Option<String>,
    preview: Option<String>,
    readme: Option<String>,
    branch: Option<String>,
    authors: Option<Vec<Author>>,
    tags: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ListOrAppManifest {
    Not(AppManifest),
    List(Vec<AppManifest>),
}

async fn search_tag(crab: &Octocrab, tag: &str) -> Vec<Repository> {
    let mut current_page = crab
        .search()
        .repositories(&format!("topic:{tag}"))
        .sort("stars")
        .order("desc")
        .per_page(100)
        .send()
        .await
        .expect("Failed to search repositories");

    let mut all_pages: Vec<Repository> = current_page.take_items();

    while let Ok(Some(mut new_page)) = crab.get_page(&current_page.next).await {
        all_pages.extend(new_page.take_items());

        current_page = new_page;
    }

    return all_pages;
}

fn filter_tag(blacklist: Vec<String>, tag: Vec<Repository>) -> Vec<Repository> {
    tag.into_iter()
        .filter(|x| {
            !blacklist.contains(&x.html_url.clone().unwrap().to_string())
                && !x.archived.unwrap_or(false)
        })
        .collect()
}

async fn get_manifest(crab: &Octocrab, repo: &Repository) -> Option<String> {
    match crab
        .repos(repo.owner.clone().unwrap().login, repo.clone().name)
        .get_content()
        .path("manifest.json")
        .send()
        .await
    {
        Ok(ok) => {
            return match ok.items.first() {
                Some(some) => return some.decoded_content(),
                None => {
                    println!(
                        "Failed to convert manifest.json to string from: {}",
                        repo.url
                    );
                    None
                }
            }
        }
        Err(..) => {
            println!("Failed to get manifest.json from: {}", repo.url);
            return None;
        }
    }
}

#[tokio::main]
async fn main() {
    let crab: Octocrab = octocrab::Octocrab::builder()
        .personal_token(std::env::var("GITHUB_TOKEN").unwrap())
        .build()
        .expect("Could not find $GITHUB_TOKEN");

    let blacklist = crab
        .repos("spicetify", "marketplace")
        .get_content()
        .path("resources/blacklist.json")
        .r#ref("main")
        .send()
        .await
        .expect("Could not get blacklist.json")
        .take_items()
        .into_iter()
        .next()
        .and_then(|x| x.decoded_content());

    let vector: Blacklist = serde_json::from_str(&blacklist.expect("Failed to read blacklist"))
        .expect("Failed to parse blacklist");

    /*
     let mut themes: Vec<Repository> =
            filter_tag(&crab, search_tag(&crab, "spicetify-themes").await?).await?;
        let mut apps: Vec<Repository> =
            filter_tag(&crab, search_tag(&crab, "spicetify-apps").await?).await?;

        let mut all: Vec<Repository> = vec![];pip mode
        all.append(&mut extensions);
        all.append(&mut themes);
        all.append(&mut apps);

        for i in all {
            println!("{}", i.url)
        }
    */

    let extensions: Vec<Repository> = filter_tag(
        vector.repos,
        search_tag(&crab, "spicetify-extensions").await,
    );

    let mut ext_manifests: Vec<ExtManifest> = vec![];
    let mut filtered_exts: Vec<&Repository> = vec![];

    for i in 0..extensions.len() {
        let manifest = match get_manifest(&crab, &extensions[i]).await {
            Some(x) => x,
            None => continue,
        };

        let parse: ListOrExtManifest = match serde_json::from_str(&manifest) {
            Ok(ok) => ok,
            Err(..) => {
                println!(
                    "Failed to parse manifest from: {}",
                    &extensions[i].html_url.clone().unwrap().to_string()
                );

                continue;
            }
        };

        match parse {
            ListOrExtManifest::Not(n) => {
                ext_manifests.push(n);
                filtered_exts.push(&extensions[i])
            }
            ListOrExtManifest::List(mut l) => {
                ext_manifests.append(&mut l);
                filtered_exts.push(&extensions[i]);
            }
        };
    }

    for i in ext_manifests {
        println!("{},", i.name.unwrap_or(String::from("unga bunga")))
    }
}
