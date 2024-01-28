
# PushDg

Wannabe roguelike, but centered around sokoban-like puzzle mechanics.

## The project vision (not yet achieved):

You don't have a sword or a shield in your inventory. You don't have an inventory. You push the sword or the shield or both in front of you across the rooms, like boxes in a sokoban. There may be HP and damages, but this dungeon crawler is more about puzzle-like carful navigation and manipulation of the environment than looking at walls of stats. You may lose all your HP, but you may also lose by ending up blocked by the puzzle-like machanics. Your *redo*s are an other kind of HP that are just as important, if not more.

## Implemented

- Pushing, respecting object mass, pusher force, and hitting with what is pushed.
- Enemies, moving when it is their turn towards the player with a trivial AI.
- Graphics, 8x8 sprites, animations, damage numbers.
- Camera, follows the player smoothly.
- Some very basic, dirty and messy procedural level generation.
- Visibility, must have line of sight and be close enough to see a tile.

## Guide

### The basic mechanics

The world is a grid of square tiles, each may contain an object, like the bunny, an enemy, a wall, a sword, a rock, etc. You are the bunny and can move in the four directions. After your move, the game lets the enemies move too, before giving the control back to you, etc. Turn by turn motion on a grid like a classic roguelike.

If the player or an enemy, let's call them the mover, attempts to move to a tile that contains an object, then the mover will attempt to push the blocking object. If the blocking object is also blocked, then the mover will attempt to push the two objects, etc. The force of the mover is a number that can only push a chain of objects whose total mass is lower or equal to it. Should the push fail, a hit occurs with the frontmost pushed object hitting the blocking object.

Not all objects can take damages, only the ones that have HP, like the player bunny or the enemies. The object that hits detremines the amount of damages dealt to the target.

Different object types have different stats, knowing those are important. The bunny has a force of 2, and most objects have a mass of 1. Most objects (including the bunny) deal 1 damage, but the sword deals 3, the shield 0, and the slime 2. The slime also has a force of 2. (These may change as the mechanics are adjusted.)

### Some advice

First, the only way to carry around and position your equipment such as swords and shields is to push them. And, as in a sokoban, pushing stuff in corners or on some walls can get them stuck, beware.

Now, suppose you are facing an enemy. Between each of your moves, the enemy may move too. If you charge and hit it repeatedly with your fists, it will have lots of occasions to hit you too and you will take a lot of damages this way (>_<'). You have to use objects.

What if you use a sword. You charge with a sword, and you have the right timing so that you land the first hit (because with the wrong timing, the enemy gets the firt hit in this kind of mele encounter). You deal 3 damages to the enemy, great! Now if the enemy is not dead, it will probably decide that walking towards you is still a good idead, and indeed, it will be, because now it is the enemy that pushes the sword into you, dealing you 3 damages too! You get it: if all that is between you and an enemy is a sword, then the sword is as much your's as it is the enemy's.

There are other objects too, let's try something else. The rock, for example, deals 1 damage, as much as you, but less than the slime, so putting the rock between you and an enemy sets the game for a fight in which the one with the most HP (+/- 1 depending on the timing) wins (and at what cost?). Not that great, we can loose lots of HP this way. What about using a shield? It deals 0 damage, so this could set the game for a fight that you can't lose nor win. (>.<')

Mmm... The problem is the symetry of the situation, you push they push and repeat with the same weapon. What if you could take the adventage by setting up an asymmetric situation with the enemy at the worst end? What if you put two objects between you and them, two different objects, so that the one that ends up hitting the enemy is not the same as the one that ends up hitting you so that you can deal more damages than you recieve?

Well, this is a working basic tactic that can get a player through some fights ^^. As more mechanics, objects, room layouts, player abilities, enemies with more behaviors and abilities, and thingies are added to the game, finer tactics with turn by turn case by case may become necessary.
