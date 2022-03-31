use parser::test_utils::parse_cfg_from_statements;

#[test]
fn cfg_test() {
    insta::assert_display_snapshot!(
        "single statement, no control flow",
        parse_cfg_from_statements(
            "
      let a = 1
      "
        )
    );

    insta::assert_display_snapshot!(
        "multiple statement, no control flow",
        parse_cfg_from_statements(
            "
      let a = 1
      let b = 1
      let c = 1
      "
        )
    );

    insta::assert_display_snapshot!(
        "single return statement",
        parse_cfg_from_statements(
            "
      return 1
      "
        )
    );

    insta::assert_display_snapshot!(
        "multiple statements, no control flow, return at end",
        parse_cfg_from_statements(
            "
      let a = 1
      let b = 2
      let c = 3
      return 1
      "
        )
    );

    insta::assert_display_snapshot!(
        "multiple statements, early return (dead code)",
        parse_cfg_from_statements(
            "
      return 1
      let a = 1
      let b = 2
      let c = 3
      "
        )
    );

    insta::assert_display_snapshot!(
        "single if statement",
        parse_cfg_from_statements(
            "
      if true {
        let x = 1
      }
      "
        )
    );

    insta::assert_display_snapshot!(
        "single if statement, single leading statement",
        parse_cfg_from_statements(
            "
      let a = 1
      if true {
        let b = 1
      }
      "
        )
    );

    insta::assert_display_snapshot!(
        "single if statement, multiple leading statements",
        parse_cfg_from_statements(
            "
      let a = 1
      let b = 1
      let c = 1
      if true {
        let d = 1
      }
      "
        )
    );

    insta::assert_display_snapshot!(
        "single if statement, single trailing statement",
        parse_cfg_from_statements(
            "
      if true {
        let d = 1
      }
      let c = 1
      "
        )
    );

    insta::assert_display_snapshot!(
        "single if statement, multiple trailing statements",
        parse_cfg_from_statements(
            "
      if true {
        let a = 1
      }
      let b = 1
      let c = 1
      let d = 1
      "
        )
    );

    insta::assert_display_snapshot!(
        "if statement, early return",
        parse_cfg_from_statements(
            "
      if true {
        return 1
      }
      let a = 1
      let b = 1
      let c = 1
    "
        )
    );

    insta::assert_display_snapshot!(
        "if statement, leading statements, early return",
        parse_cfg_from_statements(
            "
      let a = 1
      let a = 1
      if true {
        return 1
      }
      let a = 1
      let b = 1
      let c = 1
  "
        )
    );

    insta::assert_display_snapshot!(
        "single if/else statement",
        parse_cfg_from_statements(
            "
          if true {
            let a = 1
          } else {
            let a = 1
            let b = 1
          }
          "
        )
    );

    insta::assert_display_snapshot!(
        "single if/else statement, leading statements",
        parse_cfg_from_statements(
            "
        let a = 1
        let b = 1
        if true {
          let a = 1
        } else {
          let a = 1
          let b = 1
        }
        "
        )
    );

    insta::assert_display_snapshot!(
        "single if/else statement, trailing statements",
        parse_cfg_from_statements(
            "
      if true {
        let a = 1
      } else {
        let a = 1
        let b = 1
      }
      let a = 1
      let b = 1
      "
        )
    );

    insta::assert_display_snapshot!(
        "single if/else statement, leading and trailing statements",
        parse_cfg_from_statements(
            "
    let a = 1
    let b = 1
    if true {
      let a = 1
    } else {
      let a = 1
      let b = 1
    }
    let c = 1
    let d = 1
    "
        )
    );

    insta::assert_display_snapshot!(
        "single if/else statement, partial single statement early return",
        parse_cfg_from_statements(
            "
    if true {
      return 1
    } else {
      let a = 1
      let b = 1
    }
    let c = 1
    let d = 1
    "
        )
    );

    insta::assert_display_snapshot!(
        "single if/else statement, partial multiple statement early return",
        parse_cfg_from_statements(
            "
    if true {
      let a = 1
      return 1
    } else {
      let a = 1
      let b = 1
    }
    let c = 1
    let d = 1
  "
        )
    );

    insta::assert_display_snapshot!(
        "single if/else statement, full single statement early return (dead code)",
        parse_cfg_from_statements(
            "
    if true {
      return 1
    } else {
      return 2
    }
    let c = 1
    let d = 1
    "
        )
    );

    insta::assert_display_snapshot!(
        "single if/else statement, full multiple statement early return (dead code)",
        parse_cfg_from_statements(
            "
        if true {
          let a = 1
          let b = 1
          return 1
        } else {
          let a = 1
          let b = 1
          return 2
        }
        let c = 1
        let d = 1
      "
        )
    );

    insta::assert_display_snapshot!(
        "nested if statements",
        parse_cfg_from_statements(
            "
        if true {
          let a = 1
          if true {
            let a = 1
          }
          let b = 1 
        }
        "
        )
    );

    insta::assert_display_snapshot!(
        "nested if statements, leading and trailing statements",
        parse_cfg_from_statements(
            "
      let a = 1
      let b = 1
      if true {
        let a = 1
        if true {
          let a = 1
        }
        let b = 1 
      }
      let c = 1
      let d = 1
      "
        )
    );

    insta::assert_display_snapshot!(
        "nested empty if statements",
        parse_cfg_from_statements(
            "
    if true {
      if true {
        let a = 1
      }
    }
    "
        )
    );

    insta::assert_display_snapshot!(
        "nested if statements with deep return",
        parse_cfg_from_statements(
            "
    if true {
      if true {
        return 5
      }
      let x = 1
    }
    let x = 1
  "
        )
    );

    insta::assert_display_snapshot!(
        "deeply nested if statements",
        parse_cfg_from_statements(
            "
          if true {
            if true {
              if true {
                if true {
                  if true {
                    let y = 1
                  }
                }
              }
            }
          }
        "
        )
    );

    insta::assert_display_snapshot!(
        "deeply nested if statements, interleaved normal flow statements",
        parse_cfg_from_statements(
            "
              if true {
                if true {
                  let x = 1
                  if true {
                    if true {
                      let y = 1
                      let z = 1
                      if true {
                        let a = 1
                        let b = 1
                        let c = 1
                      }
                    }
                  }
                }
              }
            "
        )
    );

    insta::assert_display_snapshot!(
        "deeply nested if statements, interleaved normal flow statements, early return",
        parse_cfg_from_statements(
            "
              if true {
                if true {
                  let x = 1
                  if true {
                    if true {
                      let y = 1
                      let z = 1
                      if true {
                        return 1
                      }
                    }
                    let a = 1
                    let b = 2
                    let c = 3
                  }
                }
              }"
        )
    );

    insta::assert_display_snapshot!(
        "nested if/else statements",
        parse_cfg_from_statements(
            "
              if true {
                let a = 1
              } else {
                let a = 1
                let b = 1
                if true {
                  let a = 1
                  let b = 1
                  let c = 1
                } else {
                  let a = 1
                  let b = 1
                  let c = 1
                  let d = 1
                }
                let b = 1
              }
              "
        )
    );

    insta::assert_display_snapshot!(
        "nested if/else statements, leading and trailing statements",
        parse_cfg_from_statements(
            "
              let a = 1
              let b = 1
              if true {
                let a = 1
              } else {
                let a = 1
                let b = 1
                if true {
                  let a = 1
                  let b = 1
                  let c = 1
                } else {
                  let a = 1
                  let b = 1
                  let c = 1
                  let d = 1
                }
                let b = 1
              }
              let c = 1
              let d = 1
              "
        )
    );

    insta::assert_display_snapshot!(
        "nested if/else, early return (dead code)",
        parse_cfg_from_statements(
            "
          if true {
            if true {
              return 1
            } else {
              return 2
            }
          } else {
            if true {
              return 1
            } else {
              return 2
            }
          }
          let a = 1
          let b = 1
          let c = 1
          "
        )
    );

    insta::assert_display_snapshot!(
        "single if/else-if statement",
        parse_cfg_from_statements(
            "
          if true {
            let a = 1
          } else if true {
            let a = 1
            let b = 1
          } else {
            let a = 1
            let b = 1
            let c = 1
          }
          "
        )
    );

    insta::assert_display_snapshot!(
        "single long if/else-if statement",
        parse_cfg_from_statements(
            "
          if true {
            let a = 1
          } else if true {
            let a = 1
            let b = 1
          } else if true {
            let a = 1
            let b = 1
            let c = 1
          } else if true {
            let a = 1
            let b = 1
            let c = 1
            let d = 1
          } else {
            let a = 1
            let b = 1
            let c = 1
            let d = 1
            let e = 1
          }
          "
        )
    );

    insta::assert_display_snapshot!(
        "single if/else-if statement, early return (dead code)",
        parse_cfg_from_statements(
            "
          if true {
            return 1
          } else if true {
            return 2
          } else {
            return 3
          }
          let a = 1
          let b = 1
          let c = 1
          "
        )
    );

    insta::assert_display_snapshot!(
        "single if/else-if statement, without else block",
        parse_cfg_from_statements(
            "
           if true {
            let a = 1
           } else if true {
             let a = 1
             let b = 1
           }
          "
        )
    );

    insta::assert_display_snapshot!(
        "single if/else-if statement, without else block, early return",
        parse_cfg_from_statements(
            "
          let a = 1
           if true {
             return 1
           } else if true {
             return 2
           }
           let a = 1
           let b = 2
          "
        )
    );

    insta::assert_display_snapshot!(
        "multiple if/else-if statements",
        parse_cfg_from_statements(
            "
          if true {
            let a = 1
          } else if true {
            let a = 1
            let b = 2
          }
          if true {
            let a = 1
            let b = 2
            let c = 3
          } else if true {
            let a = 1
            let b = 2
            let c = 3
            let d = 4
          } else {
            let a = 1
            let b = 2
            let c = 3
            let d = 4
            let e = 5
          }
          let a = 1
          if true {
            let a = 1
            let b = 2
            let c = 3
          } else if true {
            let a = 1
            let b = 2
            let c = 3
            let d = 4
          } else {
            let a = 1
            let b = 2
            let c = 3
            let d = 4
            let e = 5
          }
          "
        )
    );

      insta::assert_display_snapshot!(
          "nested if/else-if statements",
          parse_cfg_from_statements(
              "
        if true {
          let a = 1
          if true {
            let a = 1
            let b = 2
          } else if true {
            let a = 1
            let b = 2
            let c = 3
          }
        } else if true {
          let a = 1
          let b = 2
          let c = 3
          let d = 4
          if true {
            let a = 1
          } else {
            let c = 2
          }
        } else {
          if true {
            let a = 1
          } else if true {
            let b = 1
            let c = 2
          }
          let a = 1
          let b = 2
          let c = 3
          let d = 4
          let e = 5
        }
        "
          )
      );

      insta::assert_display_snapshot!(
          "nested if/else-if statements, early return (dead code)",
          parse_cfg_from_statements(
              "
        if true {
          if true {
            return 5
          } else if true {
            return 5
          } else {
            if true {
              return 5
            } else if true {
              return 5
            } else {
              let a = 1
              return 5
            }
            # DEAD CODE
            let a = 1
          }
        } else if true {
          if true {
            return 5
          } else if true {
            return 5
          } else {
            return 5
          }
          # DEAD CODE
          let a = 1
        } else {
          if true {
            return 5
          } else if true {
            return 5
          } else {
            return 5
          }
          # DEAD CODE
          let a = 1
        }
        # DEAD CODE
        let a = 1
        let b = 1
        let c = 1
        "
          )
      );
}

#[test]
fn while_cfg_snapshots() {
    insta::assert_display_snapshot!(
      "single while statement",
      parse_cfg_from_statements(
        "
        while true {
          let a = 1
        }
        "
      )
    );

    insta::assert_display_snapshot!(
      "single while statement, trailing statement",
      parse_cfg_from_statements(
        "
        while true {
          let a = 1
        }
        let a = 1
        let b = 1
        "
      )
    );

    insta::assert_display_snapshot!(
      "single while statement, leading statement",
      parse_cfg_from_statements(
        "
        let a = 1
        let a = 1
        while true {
          let a = 1
        }
        "
      )
    );
}
