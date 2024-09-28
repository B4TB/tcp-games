#!/usr/bin/env python3

# This file implements the kind of frivolous nonsense we'll be doing: To get a
# feel for the code we're going to write, you may find it useful to read this
# top-to-bottom.

# We'll start by importing the things we'll need to write our game. Asyncio
# lets us open and use network connections.
import asyncio

# The computer's moves are determined randomly.
import random

# We'll run our server on the given host and port. The host needs to
# be 0.0.0.0 to be accessible from other machines, but the port is
# arbitrary.
HOST = '0.0.0.0'
PORT = 5775

# The first section of code here contains game logic for nim. If you're
# unfamiliar, the [wikipedia page](https://en.wikipedia.org/wiki/Nim) is a good
# resource.

class Nim:
    def __init__(self):
        """We start our game with 3 objects in the first pile, 4 in the second,
        and 5 in the third. I believe these are called nimbers, and are somehow
        unreasonably important to math. I don't know SHIT, ask Tristan."""
        self._piles = [3, 4, 5]

    def move_is_valid(self, pile, amount):
        return amount > 0 and \
            pile < len(self._piles) and \
            self._piles[pile] >= amount

    def play(self, pile, amount):
        """Take `amount` objects from the specified `pile`."""
        assert(self.move_is_valid(pile, amount))
        self._piles[pile] -= amount

    def can_continue(self):
        """Return True iff the game can continue."""
        return sum(self._piles) != 0

    def random_move(self):
        pile = random.choice([i for i, x in enumerate(self._piles) if x > 0])
        amount = random.randint(1, self._piles[pile])
        return pile, amount

    def __str__(self):
        out = ''
        char = '*'
        for i, pile_amount in enumerate(self._piles):
             out += f'\x1b[1m[{i}]\x1b[0m \x1b[33m{char * pile_amount}\x1b[0m\n'

        return out

class NimDriver:
    def __init__(self, reader, writer):
        """Drive a game of [Nim] with the given socket."""
        self._reader = reader
        self._writer = writer
        self._nim = Nim()

    async def play(self):
        while await self._round(): pass

    async def _round(self):
        """Ask for a move, do it, and respond with a move of our own."""
        self._writer.write(str(self._nim).encode())
        await self._writer.drain()

        pile, amount = await self._ask_move()
        await self._writer.drain()
        while not self._nim.move_is_valid(pile, amount):
            self._writer.write('Not a valid move.\n'.encode())
            pile, amount = await self._ask_move()
            await self._writer.drain()

        self._nim.play(pile, amount)

        self._writer.write(str(self._nim).encode())
        await self._writer.drain()
        await self._fake_progress_bar('Determining response')

        if not self._nim.can_continue():
            self._writer.write('You won. Congrats.\n'.encode())
            await self._writer.drain()
            return False

        pile, amount = self._nim.random_move()
        self._nim.play(pile, amount)

        if not self._nim.can_continue():
            self._writer.write(str(self._nim).encode())

            self._writer.write('\nThe computer beat you. That\'s honestly kinda sad.\n'.encode())

            await self._writer.drain()
            return False

        return True

    async def _ask_move(self):
        pile = await self._read_int('Which pile would you like to take from?')
        amount = await self._read_int('How many would you like to take?')
        return pile, amount

    async def _read_int(self, prompt):
        while True:
            self._writer.write(f'{prompt}\n> '.encode())
            await self._writer.drain()

            line = await self._reader.readline()
            line.strip()
            try:
                v = int(line)
                return v
            except ValueError:
                self._writer.write('Input wasn\'t a valid integer.\n'.encode())
                await self._writer.drain()

    async def _fake_progress_bar(self, prompt):
        for i in range(0, 15):
             dots = ('*' * (i % 3 + 1)).rjust(3)
             self._writer.write(f'\r{prompt}: {dots}'.encode())
             await self._writer.drain()
             await asyncio.sleep(0.1)

        self._writer.write('\n'.encode())


async def main():
    async def cb(reader, writer):
         driver = NimDriver(reader, writer)
         await driver.play()
         writer.close()
         await writer.wait_closed()

    server = await asyncio.start_server(
        cb, host=HOST, port=PORT, start_serving=False
    )

    print(f'Serving on {HOST}:{PORT}')

    await server.serve_forever()

if __name__ == '__main__':
    # Finally, we run our code.
    asyncio.run(main())
