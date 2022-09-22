/**
 * GENERATED FILE - DO NOT EDIT
 * Compiled from module: main.ws
 * Generated at 2022-09-22T04:13:48.455276+00:00
 */
import { signal } from "@preact/signals-core";
function main$App$create_fragment_5(count, handleClick) {
  let $1;
  let $2;
  let $3;
  let $4;
  let $6;
  let $7;
  let $9;
  let $10;

  return {
    create() {
      $1 = document.createElement("div");
      $2 = document.createElement("span");
      $3 = document.createTextNode(count.value);
      $4 = document.createElement("button");
      $4.addEventListener("click", handleClick);
      $5 = document.createTextNode("Click me");
      $6 = document.createElement("div");
      $7 = document.createElement("h1");
      $8 = document.createTextNode("Hello, ");
      $9 = document.createElement("span");
      $9.setAttribute("style", "color: red");
      $10 = document.createTextNode(count.value);

      // Subscriptions
      count.subscribe((v) => {
        $3.textContent = v;
        $10.textContent = v;
      });
    },
    mount(target) {
      target.appendChild($1);
      $1.appendChild($2);
      $2.appendChild($3);
      $1.appendChild($4);
      $4.appendChild($5);
      $1.appendChild($6);
      $6.appendChild($7);
      $7.appendChild($8);
      $7.appendChild($9);
      $9.appendChild($10);
    },
  };
}
export class App {
  constructor() {
    let count = signal(0);
    function handleClick() {
      if (count.value < 10) {
        count.value = count.value + 1;
      }
    }
    return main$App$create_fragment_5(count, handleClick);
  }
}
