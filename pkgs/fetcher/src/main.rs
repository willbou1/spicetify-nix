use octocrab::{models::Repository, Octocrab};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, OneOrMany};
use std::{collections::HashMap, fs::File, process::Command};

const NIX: &str = "/run/current-system/sw/bin/nix";

#[derive(Serialize, Deserialize)]
struct Blacklist {
    repos: Vec<String>,
}
#[serde_as]
#[derive(Clone, Serialize, Deserialize, Debug)]
struct ExtManifests(#[serde_as(as = "OneOrMany<_>")] Vec<ExtManifest>);

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ExtManifest {
    name: String,
    main: String,
    branch: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ExtOutput {
    name: String,
    main: String,
    source: FetchFromGitHub,
}

#[derive(Serialize, Deserialize)]
struct ExtTuple {
    manifests: ExtManifests,
    repo: Repository,
}

#[derive(Serialize, Deserialize)]
struct Output {
    extensions: Vec<ExtOutput>,
}

#[derive(Serialize, Deserialize)]
struct Prefetch {
    hash: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct FetchFromGitHub {
    owner: String,
    repo: String,
    hash: String,
    rev: String,
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
            !blacklist.contains(
                &x.html_url
                    .clone()
                    .expect("Epic html_url failure")
                    .to_string(),
            ) && !x.archived.unwrap_or(false)
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
        .personal_token(std::env::var("GITHUB_TOKEN").expect("no PAT key moron"))
        .build()
        .expect("Failed to crab");

    let blacklist = crab
        .repos("spicetify", "marketplace")
        .get_content()
        .path("resources/blacklist.json")
        .r#ref("main")
        .send()
        .await
        .expect("Could not get blacklist.json")
        .items
        .first()
        .unwrap()
        .decoded_content();

    let vector: Blacklist = serde_json::from_str(&blacklist.expect("Failed to read blacklist"))
        .expect("Failed to parse blacklist");

    let extensions: Vec<Repository> = filter_tag(
        vector.repos,
        search_tag(&crab, "spicetify-extensions").await,
    );

    let mut ext_tuple: Vec<ExtTuple> = vec![];

    for i in 0..extensions.len() {
        let manifest = match get_manifest(&crab, &extensions[i]).await {
            Some(x) => x,
            None => continue,
        };

        let parse: ExtManifests = match serde_json::from_str(&manifest) {
            Ok(ok) => ok,
            Err(..) => {
                println!(
                    "Failed to parse manifest from: {}",
                    &extensions[i].html_url.clone().unwrap().to_string()
                );

                continue;
            }
        };

        ext_tuple.push(ExtTuple {
            manifests: parse,
            repo: extensions[i].clone(),
        });
    }

    let mut potato: HashMap<String, FetchFromGitHub> = HashMap::new();
    let mut potato2: HashMap<String, ExtOutput> = HashMap::new();

    for i in ext_tuple {
        let owner = &i
            .repo
            .owner
            .clone()
            .expect("failed to get repo owner?")
            .login;
        let name = &i.repo.name;

        for j in i.manifests.0 {
            let branch = j.branch.unwrap_or(
                i.repo
                    .default_branch
                    .clone()
                    .expect("failed to get default_branch"),
            );
            let rev = crab
                .commits(owner, name)
                .get(branch.clone())
                .await
                .expect("failed to get latest commit of branch")
                .sha;

            let file = format!(
                "{}/archive/{}.tar.gz",
                i.repo.html_url.clone().expect("Epic html_url failure"),
                rev
            );

            let command_stdout = Command::new(NIX)
                .args(["store", "prefetch-file", &file, "--json"])
                .output()
                .expect("failed to run nix store prefetch-file lol")
                .stdout;
            let prefetched: Prefetch = serde_json::from_str(&String::from_utf8_lossy(
                &command_stdout,
            ))
            .expect("failed to parse nix store prefetch-file output, how did you make this fail?");

            let key = format!("{}-{}-{}", owner, name, branch);

            if potato.get(&key).is_none() {
                println!("{}", file);
                potato.insert(
                    key.clone(),
                    FetchFromGitHub {
                        owner: owner.to_string(),
                        repo: name.to_string(),
                        rev,
                        hash: prefetched.hash.to_string(),
                    },
                );
            };

            potato2.insert(
                j.name.clone(),
                ExtOutput {
                    name: j.name,
                    source: potato.get(&key).unwrap().clone(),
                    main: j.main,
                },
            );
        }
    }

    let file2 = File::create("extensions.json").expect("can't create extensions.jspon");
    serde_json::to_writer_pretty(&file2, &potato2).expect("failed to save extensions.json");
}
