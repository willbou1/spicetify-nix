use octocrab::{models::Repository, Octocrab};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, OneOrMany};
use std::{collections::HashMap, fs::File, process::Command};

const NIX: &str = "/run/current-system/sw/bin/nix";

#[derive(Serialize, Deserialize)]
struct Blacklist {
    repos: Vec<String>,
}
#[derive(Serialize, Deserialize)]
struct Prefetch {
    hash: String,
}
#[derive(Serialize, Deserialize, Clone)]
struct FetchURL {
    url: String,
    hash: String,
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
struct ExtTuple {
    manifests: ExtManifests,
    repo: Repository,
}

#[derive(Serialize, Deserialize)]
struct ExtOutput {
    name: String,
    main: String,
    source: FetchURL,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AppManifest {
    name: String,
    branch: Option<String>,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize, Debug)]
struct AppManifests(#[serde_as(as = "OneOrMany<_>")] Vec<AppManifest>);

#[derive(Serialize, Deserialize)]
struct AppTuple {
    manifests: AppManifests,
    repo: Repository,
}

#[derive(Serialize, Deserialize)]
struct AppOutput {
    name: String,
    source: FetchURL,
}

#[derive(Serialize, Deserialize)]
struct Output {
    extensions: HashMap<String, ExtOutput>,
    apps: HashMap<String, AppOutput>,
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

fn get_owner(repo: &Repository) -> String {
    repo.owner.clone().expect("failed to get repo owner?").login
}

fn get_default_branch(repo: &Repository) -> String {
    repo.default_branch
        .clone()
        .expect("failed to get default_branch")
}

async fn get_rev(crab: &Octocrab, owner: &str, name: &str, branch: &String) -> Option<String> {
    match crab.commits(owner, name).get(branch.clone()).await {
        Ok(x) => Some(x.sha),
        Err(..) => {
            println!(
                "Failed to get latest commit of github.com/{}/{} branch: {}",
                owner, name, branch
            );
            None
        }
    }
}

fn fetch_url(repo: &Repository, rev: String) -> FetchURL {
    let file = format!(
        "{}/archive/{}.tar.gz",
        repo.html_url.clone().expect("Epic html_url failure"),
        rev
    );
    println!("{}", file);
    let command_stdout = Command::new(NIX)
        .args(["store", "prefetch-file", &file, "--json"])
        .output()
        .expect("failed to run nix store prefetch-file lol")
        .stdout;
    let prefetched: Prefetch = serde_json::from_str(&String::from_utf8_lossy(&command_stdout))
        .expect("failed to parse nix store prefetch-file output, how did you make this fail?");

    FetchURL {
        url: file,
        hash: prefetched.hash,
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
        vector.repos.clone(),
        search_tag(&crab, "spicetify-extensions").await,
    );

    let mut potato: HashMap<String, FetchURL> = HashMap::new();

    // Extension stuff
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
    let mut ext_outputs: HashMap<String, ExtOutput> = HashMap::new();
    for i in ext_tuple {
        let owner = &get_owner(&i.repo);
        let name = &i.repo.name;

        for j in i.manifests.0 {
            let branch = j.branch.unwrap_or(get_default_branch(&i.repo));

            let rev = match get_rev(&crab, owner, name, &branch).await {
                Some(x) => x,
                None => continue,
            };

            let key = format!("{}-{}-{}", owner, name, branch);

            if potato.get(&key).is_none() {
                potato.insert(key.clone(), fetch_url(&i.repo, rev));
            };

            ext_outputs.insert(
                j.name.clone(),
                ExtOutput {
                    name: j.name,
                    source: potato.get(&key).unwrap().clone(),
                    main: j.main,
                },
            );
        }
    }
    // App stuff

    let apps: Vec<Repository> = filter_tag(vector.repos, search_tag(&crab, "spicetify-apps").await);

    let mut app_tuple: Vec<AppTuple> = vec![];
    for i in 0..apps.len() {
        let manifest = match get_manifest(&crab, &apps[i]).await {
            Some(x) => x,
            None => continue,
        };

        let parse: AppManifests = match serde_json::from_str(&manifest) {
            Ok(ok) => ok,
            Err(..) => {
                println!(
                    "Failed to parse manifest from: {}",
                    &apps[i].html_url.clone().unwrap().to_string()
                );

                continue;
            }
        };

        app_tuple.push(AppTuple {
            manifests: parse,
            repo: apps[i].clone(),
        });
    }

    let mut app_outputs: HashMap<String, AppOutput> = HashMap::new();

    for i in app_tuple {
        let owner = &get_owner(&i.repo);
        let name = &i.repo.name;

        for j in i.manifests.0 {
            let branch = j.branch.unwrap_or(get_default_branch(&i.repo));

            let rev = match get_rev(&crab, owner, name, &branch).await {
                Some(x) => x,
                None => continue,
            };

            let key = format!("{}-{}-{}", owner, name, branch);

            if potato.get(&key).is_none() {
                potato.insert(key.clone(), fetch_url(&i.repo, rev));
            };

            app_outputs.insert(
                j.name.clone(),
                AppOutput {
                    name: j.name,
                    source: potato.get(&key).unwrap().clone(),
                },
            );
        }
    }

    let final_output: Output = Output {
        extensions: ext_outputs,
        apps: app_outputs,
    };

    let output = File::create("generated.json").expect("can't create generated.json");
    serde_json::to_writer_pretty(&output, &final_output).expect("failed to save generated.json");
}
