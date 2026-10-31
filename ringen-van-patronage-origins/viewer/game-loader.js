const message = document.querySelector("#game-message");

try {
  const { default: startGame } = await import(`../game-build/ringen_game.js?v=${Date.now()}`);
  await startGame();
  message.remove();
  document.querySelector("#game-canvas").focus();
} catch (error) {
  message.classList.add("error");
  message.textContent = `The browser build is not ready yet. Reload the page, or use “Recompile game” in the local studio. ${error.message}`;
}
