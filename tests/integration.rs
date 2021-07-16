use assert_cmd::prelude::*;

use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn publish_doesnt_nuke_kv() {
    let mut wrangler = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    wrangler
        .arg("generate")
        .arg("--site")
        .arg("test_site")
        .assert()
        .success();
    assert_eq!(Path::new("tests/test_wrangler.toml").exists(), true);
    fs::copy("tests/test_wrangler.toml", "test_site/wrangler.toml").unwrap();
    let mut wrangler = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    wrangler
        .current_dir("./test_site")
        .arg("publish")
        .assert()
        .success();

    Command::new("bash")
        .arg("-c")
        .arg("source tests/curlkv.sh")
        .spawn()
        .expect(
            "{
        \"result\": [
          {
            \"name\": \"404.94a1fd7cd6.html\"
          },
          {
            \"name\": \"favicon.ff38969f14.ico\"
          },
          {
            \"name\": \"img/200-wrangler-ferris.8f4194bc08.gif\"
          },
          {
            \"name\": \"img/404-wrangler-ferris.8256cc7e19.gif\"
          },
          {
            \"name\": \"index.5dc8b03172.html\"
          }
        ],
        \"success\": true,
        \"errors\": [],
        \"messages\": [],
        \"result_info\": {
          \"count\": 5,
          \"cursor\": \"\"
        }
        }",
        );
        cleanup("test_site");
}

fn cleanup(name: &str) {
    fs::remove_dir_all(name).unwrap();
    assert_eq!(Path::new(name).exists(), false);
}