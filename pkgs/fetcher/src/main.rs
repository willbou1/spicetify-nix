use octocrab::models::Repository;
use std::error::Error;

async fn search_tag(tag: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let octocrab = octocrab::Octocrab::builder()
        .personal_token(std::env::var("GITHUB_TOKEN").unwrap())
        .build()?;

    let mut current_page = octocrab
        .search()
        .repositories(&format!("topic:{tag}"))
        .sort("stars")
        .order("desc")
        .per_page(100)
        .send()
        .await?;

    let mut all_pages: Vec<Repository> = current_page.take_items();

    while let Ok(Some(mut new_page)) = octocrab.get_page(&current_page.next).await {
        all_pages.extend(new_page.take_items());

        current_page = new_page;
    }
    //return Ok(all_pages)
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let _ = search_tag("spicetify-extensions").await;
    let _ = search_tag("spicetify-themes").await;
    let _ = search_tag("spicetify-apps").await;
    Ok(())
}
