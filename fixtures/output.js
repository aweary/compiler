/**
 * GENERATED FILE - DO NOT EDIT
 * Compiled from module: main.ws
 * Generated at 2022-09-17T18:37:07.085827+00:00
 */
function main$Button$create_fragment_0(label, onPress) {
  let $1;

  return {
    create() {
      $1 = document.createElement("button");
      $1.addEventListener("click", onPress);
      $1.appendChild(document.createTextNode(label));
    },
    mount(target) {
      target.appendChild($1);
    },
  };
}
export class Button {
  constructor(label) {
    let count = $$state(0);
    function onPress() {
      count.set(count + 1);
    }
    return main$Button$create_fragment_0(label, onPress);
  }
}
function main$App$create_fragment_1() {
  let $1;

  return {
    create() {
      $1 = document.createElement("div");
      $1.textContent += "Hello World";
    },
    mount(target) {
      target.appendChild($1);
    },
  };
}
export class App {
  constructor() {
    return main$App$create_fragment_1();
  }
}
