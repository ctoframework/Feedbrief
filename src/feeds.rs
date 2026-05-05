#[derive(Debug, Clone)]
pub struct FeedSource {
    pub name: &'static str,
    pub url: &'static str,
    pub category: &'static str,
}

pub fn default_feeds() -> Vec<FeedSource> {
    vec![
        FeedSource { name: "Hugging Face Blog", url: "https://huggingface.co/blog/feed.xml", category: "AI/ML" },
        FeedSource { name: "Google DeepMind", url: "https://deepmind.google/blog/rss.xml", category: "AI/ML" },
        FeedSource { name: "OpenAI Blog", url: "https://openai.com/blog/rss.xml", category: "AI/ML" },
        FeedSource { name: "Anthropic News", url: "https://www.anthropic.com/news/rss.xml", category: "AI/ML" },
        FeedSource { name: "BAIR Blog", url: "https://bair.berkeley.edu/blog/feed.xml", category: "AI/ML" },
        FeedSource { name: "Import AI", url: "https://importai.substack.com/feed", category: "AI/ML" },
        FeedSource { name: "arXiv cs.AI", url: "http://export.arxiv.org/rss/cs.AI", category: "Research" },
        FeedSource { name: "arXiv cs.LG", url: "http://export.arxiv.org/rss/cs.LG", category: "Research" },
        FeedSource { name: "arXiv cs.CL", url: "http://export.arxiv.org/rss/cs.CL", category: "Research" },
        FeedSource { name: "arXiv cs.CR", url: "http://export.arxiv.org/rss/cs.CR", category: "Research" },
        FeedSource { name: "arXiv cs.RO", url: "http://export.arxiv.org/rss/cs.RO", category: "Research" },
        FeedSource { name: "TechCrunch", url: "https://techcrunch.com/feed/", category: "Startups" },
        FeedSource { name: "Hacker News Front", url: "https://hnrss.org/frontpage", category: "Startups" },
        FeedSource { name: "Y Combinator Blog", url: "https://www.ycombinator.com/blog/rss", category: "Startups" },
        FeedSource { name: "Crunchbase News", url: "https://news.crunchbase.com/feed/", category: "Startups" },
        FeedSource { name: "SemiAnalysis", url: "https://www.semianalysis.com/feed", category: "Hardware" },
        FeedSource { name: "IEEE Spectrum", url: "https://spectrum.ieee.org/feeds/feed.rss", category: "Hardware" },
        FeedSource { name: "AnandTech", url: "https://www.anandtech.com/rss/", category: "Hardware" },
        FeedSource { name: "Krebs on Security", url: "https://krebsonsecurity.com/feed/", category: "Security" },
        FeedSource { name: "The Hacker News", url: "https://feeds.feedburner.com/TheHackersNews", category: "Security" },
        FeedSource { name: "Schneier on Security", url: "https://www.schneier.com/feed/atom/", category: "Security" },
        FeedSource { name: "Ars Technica", url: "https://feeds.arstechnica.com/arstechnica/index", category: "Tech" },
        FeedSource { name: "The Verge", url: "https://www.theverge.com/rss/index.xml", category: "Tech" },
        FeedSource { name: "MIT Tech Review", url: "https://www.technologyreview.com/feed/", category: "Tech" },
        FeedSource { name: "Wired", url: "https://www.wired.com/feed/rss", category: "Tech" },
        FeedSource { name: "Quanta Magazine", url: "https://www.quantamagazine.org/feed/", category: "Science" },
        FeedSource { name: "Nature News", url: "https://www.nature.com/nature.rss", category: "Science" },
        FeedSource { name: "Science News", url: "https://www.sciencenews.org/feed", category: "Science" },
    ]
}
