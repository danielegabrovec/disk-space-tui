# Installer Windows

Script Inno Setup riproducibile. I pacchetti si pubblicano nelle GitHub Releases e non nella cronologia Git. Richiede Inno Setup 6 o successivo; non eseguire l'installer per ricompilare il progetto.

Gli installer sono per l'utente corrente, non richiedono privilegi amministrativi e non rimuovono i dati applicativi nelle cartelle personali. Non sono firmati Authenticode.

Compila `windows.iss` passando `/DPayloadDir=<cartella portabile>` e `/DReleaseDir=<output>`. Il payload deve provenire dalla versione indicata dal progetto, completo di licenze e documentazione.
